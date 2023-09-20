extern crate chrono;
extern crate nix;
extern crate timer;
use crate::occlum_exec::{
    ExecCommRequest, ExecCommResponse, ExecCommResponse_ExecutionStatus, GetResultRequest,
    GetResultResponse, GetResultResponse_ExecutionStatus, HealthCheckRequest, HealthCheckResponse,
    HealthCheckResponse_ServingStatus, KillProcessRequest, KillProcessResponse, StopRequest,
    StopResponse,
};
use crate::occlum_exec_grpc::OcclumExec;
use grpc::{ServerHandlerContext, ServerRequestSingle, ServerResponseUnarySink};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use sendfd::RecvWithFd;
use std::cmp;
use std::collections::HashMap;
use std::ffi::CString;
use std::mem;
use std::os::unix::io::RawFd;
use std::os::unix::net::UnixStream;
use std::panic;
use std::ptr;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use timer::{Guard, Timer};

pub enum ServerStatus {
    Stopped,
    Running,
    Error,
}

impl Default for ServerStatus {
    fn default() -> Self {
        Self::Stopped
    }
}

impl ServerStatus {
    pub fn set_error(&mut self) {
        *self = Self::Error
    }

    pub fn set_running(&mut self) {
        *self = Self::Running
    }

    fn set_stopped(&mut self) {
        *self = Self::Stopped
    }

    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    fn is_error(&self) -> bool {
        matches!(self, Self::Error)
    }

    fn is_stopped(&self) -> bool {
        matches!(self, Self::Stopped)
    }
}

#[derive(Default)]
pub struct OcclumExecImpl {
    //process_id, return value, execution status
    commands: Arc<Mutex<HashMap<i32, (Option<i32>, bool)>>>,
    execution_lock: Arc<(Mutex<ServerStatus>, Condvar)>,
    stop_timer: Arc<Mutex<Option<(Timer, Guard)>>>,
}

impl OcclumExecImpl {
    pub fn new_and_save_execution_lock(
        lock: Arc<(Mutex<ServerStatus>, Condvar)>,
    ) -> OcclumExecImpl {
        OcclumExecImpl {
            commands: Default::default(),
            execution_lock: lock,
            stop_timer: Arc::new(Mutex::new(None)),
        }
    }
}

impl OcclumExec for OcclumExecImpl {
    fn kill_process(
        &self,
        _o: ::grpc::ServerHandlerContext,
        mut req: ::grpc::ServerRequestSingle<KillProcessRequest>,
        resp: ::grpc::ServerResponseUnarySink<KillProcessResponse>,
    ) -> ::grpc::Result<()> {
        let req = req.take_message();
        if rust_occlum_pal_kill(req.process_id, req.signal).is_err() {
            warn!("failed to send signal to process.");
        }

        resp.finish(KillProcessResponse {
            ..Default::default()
        })
    }

    fn get_result(
        &self,
        _o: ServerHandlerContext,
        mut req: ServerRequestSingle<GetResultRequest>,
        resp: ServerResponseUnarySink<GetResultResponse>,
    ) -> grpc::Result<()> {
        let process_id = req.take_message().process_id;
        let commands = self.commands.clone();
        let mut commands = commands.lock().unwrap();
        let (process_status, result) = match &commands.get(&process_id) {
            None => (GetResultResponse_ExecutionStatus::UNKNOWN, -1),
            Some(&(exit_status, _)) => {
                match exit_status {
                    None => (GetResultResponse_ExecutionStatus::RUNNING, -1),
                    Some(return_value) => {
                        //Remove the process when getting the return value
                        commands.remove(&process_id);
                        (GetResultResponse_ExecutionStatus::STOPPED, return_value)
                    }
                }
            }
        };
        drop(commands);

        resp.finish(GetResultResponse {
            status: process_status,
            result: result,
            ..Default::default()
        })
    }

    fn stop_server(
        &self,
        _o: ServerHandlerContext,
        mut req: ServerRequestSingle<StopRequest>,
        resp: ServerResponseUnarySink<StopResponse>,
    ) -> grpc::Result<()> {
        if rust_occlum_pal_kill(-1, SIGTERM).is_err() {
            warn!("SIGTERM failed.");
        }
        let time = cmp::min(req.take_message().time, crate::DEFAULT_SERVER_TIMER);

        // New a timer to stop the server
        // If no new commands comes from the client, the SIGKILL would be send to all the process.
        // After that, the enclave would be destroyed and the server itself would exit.
        // If one status query command or execute new command request comes from client, and at that
        // time the timer is still waiting, the timer would be cancelled.
        let lock = self.execution_lock.clone();
        let timer = timer::Timer::new();
        let guard = timer.schedule_with_delay(chrono::Duration::seconds(time as i64), move || {
            if rust_occlum_pal_kill(-1, SIGKILL).is_err() {
                warn!("SIGKILL failed.")
            }
            let (execution_lock, cvar) = &*lock;
            execution_lock.lock().unwrap().set_stopped();
            cvar.notify_one();
        });

        // We could not drop the timer and guard until timer is triggered.
        *self.stop_timer.lock().unwrap() = Some((timer, guard));

        resp.finish(StopResponse::default())
    }

    fn status_check(
        &self,
        _o: ServerHandlerContext,
        _req: ServerRequestSingle<HealthCheckRequest>,
        resp: ServerResponseUnarySink<HealthCheckResponse>,
    ) -> grpc::Result<()> {
        // Clear the timer for we need the server continue service
        *self.stop_timer.lock().unwrap() = None;

        //Waits for the Occlum loaded
        let (status, _) = &*self.execution_lock.clone();
        loop {
            let server_status = status.lock().unwrap();
            if server_status.is_stopped() {
                drop(server_status);
                continue;
            }

            if server_status.is_error() {
                return Err(grpc::Error::Other("server error"));
            }

            break;
        }

        resp.finish(HealthCheckResponse {
            status: HealthCheckResponse_ServingStatus::SERVING,
            ..Default::default()
        })
    }

    fn exec_command(
        &self,
        _o: ServerHandlerContext,
        mut req: ServerRequestSingle<ExecCommRequest>,
        resp: ServerResponseUnarySink<ExecCommResponse>,
    ) -> grpc::Result<()> {
        // Clear the timer for we need the server continue service
        *self.stop_timer.lock().unwrap() = None;

        let req = req.take_message();

        //Get the client stdio
        let mut stdio_fds = occlum_stdio_fds {
            stdin_fd: 0,
            stdout_fd: 0,
            stderr_fd: 0,
        };

        match UnixStream::connect(req.sockpath) {
            Ok(stream) => {
                let mut data = [0; 10];
                let mut fdlist: [RawFd; 3] = [0; 3];
                stream
                    .recv_with_fd(&mut data, &mut fdlist)
                    .expect("receive fd failed");

                stdio_fds.stdin_fd = fdlist[0];
                stdio_fds.stdout_fd = fdlist[1];
                stdio_fds.stderr_fd = fdlist[2];
            }
            Err(e) => {
                info!("Failed to connect: {}", e);
                return resp.finish(ExecCommResponse {
                    process_id: 0,
                    ..Default::default()
                });
            }
        };

        let _commands = self.commands.clone();
        let _execution_lock = self.execution_lock.clone();

        let cmd = req.command.clone();
        let args = req.parameters.into_vec().clone();
        let envs = req.enviroments.into_vec().clone();
        let client_process_id = req.process_id;

        if let Ok(process_id) = rust_occlum_pal_create_process(&cmd, &args, &envs, &stdio_fds) {
            let mut commands = _commands.lock().unwrap();
            commands.entry(process_id).or_insert((None, true));
            drop(commands);

            // Run the command in a thread
            // Use a 8MB stack for rust started thread
            const DEFAULT_STACK_SIZE: usize = 8 * 1024 * 1024;
            thread::Builder::new()
                .stack_size(DEFAULT_STACK_SIZE)
                .spawn(move || {
                    let mut exit_status = Box::new(0);

                    let result = rust_occlum_pal_exec(process_id, &mut exit_status);
                    let mut commands = _commands.lock().unwrap();

                    if result == Ok(()) {
                        *commands.get_mut(&process_id).expect("get process") =
                            (Some(*exit_status), false);
                    } else {
                        // Return -1 if the process crashed or get any unexpected error
                        *commands.get_mut(&process_id).expect("get process") = (Some(-1), false);
                    }

                    //Notifies the client that the application stopped
                    debug!(
                        "process:{} finished, send signal to {}",
                        process_id, client_process_id
                    );
                    signal::kill(Pid::from_raw(client_process_id as i32), Signal::SIGUSR1)
                        .unwrap_or_default();
                });

            resp.finish(ExecCommResponse {
                status: ExecCommResponse_ExecutionStatus::RUNNING,
                process_id: process_id,
                ..Default::default()
            })
        } else {
            resp.finish(ExecCommResponse {
                status: ExecCommResponse_ExecutionStatus::LAUNCH_FAILED,
                process_id: 0,
                ..Default::default()
            })
        }
    }
}

/*
 * The struct which consists of file descriptors of standard I/O
 */
#[repr(C)]
pub struct occlum_stdio_fds {
    pub stdin_fd: i32,
    pub stdout_fd: i32,
    pub stderr_fd: i32,
}

/*
 * The struct which consists of arguments needed by occlum_pal_create_process
 */
#[repr(C)]
pub struct occlum_pal_create_process_args {
    pub path: *const libc::c_char,
    pub argv: *const *const libc::c_char,
    pub env: *const *const libc::c_char,
    pub stdio: *const occlum_stdio_fds,
    pub pid: *mut i32,
}

/*
 * The struct which consists of arguments needed by occlum_pal_exec
 */
#[repr(C)]
pub struct occlum_pal_exec_args {
    pub pid: i32,
    pub exit_value: *mut i32,
}

extern "C" {
    /*
     * @brief Create a new process inside the Occlum enclave
     *
     * @param args  Mandatory input. Arguments for occlum_pal_create_process.
     *
     * @retval If 0, then success; otherwise, check errno for the exact error type.
     */
    fn occlum_pal_create_process(args: *const occlum_pal_create_process_args) -> i32;

    /*
     * @brief Execute the process inside the Occlum enclave
     *
     * @param args  Mandatory input. Arguments for occlum_pal_exec.
     *
     * @retval If 0, then success; otherwise, check errno for the exact error type.
     */
    fn occlum_pal_exec(args: *const occlum_pal_exec_args) -> i32;

    /*
     * @brief Send a signal to one or multiple LibOS processes
     *
     * @param pid   If pid > 0, send the signal to the process with the
     *              pid; if pid == -1, send the signal to all processes.
     * @param sig   The signal number. For the purpose of security, the
     *              only allowed signals for now are SIGKILL and SIGTERM.
     *
     * @retval If 0, then success; otherwise, check errno for the exact error type.
     */
    fn occlum_pal_kill(pid: i32, sig: i32) -> i32;
}

pub unsafe fn disable_sigstack() {
    let mut stack: libc::stack_t = mem::zeroed();
    stack.ss_flags = libc::SS_DISABLE;
    libc::sigaltstack(&stack, ptr::null_mut());
}

fn vec_strings_to_cchars(
    strings: &Vec<String>,
) -> Result<(Vec<*const libc::c_char>, Vec<CString>), i32> {
    let mut strings_content = Vec::<CString>::new();
    let mut cchar_strings = Vec::<*const libc::c_char>::new();
    for string in strings {
        let string = CString::new(string.as_str()).expect("arg: new failed");
        cchar_strings.push(string.as_ptr());
        strings_content.push(string);
    }

    cchar_strings.push(0 as *const libc::c_char);
    Ok((cchar_strings, strings_content))
}

/// Executes the command inside Occlum enclave
fn rust_occlum_pal_create_process(
    cmd: &str,
    args: &Vec<String>,
    envs: &Vec<String>,
    stdio: &occlum_stdio_fds,
) -> Result<i32, i32> {
    let cmd_path = CString::new(cmd).expect("cmd_path: new failed");
    let (cmd_args_array, _cmd_args) = vec_strings_to_cchars(args)?;
    let (cmd_envs_array, _cmd_envs) = vec_strings_to_cchars(envs)?;

    let stdio_raw = Box::new(stdio);
    let mut libos_tid = 0;
    let argv = cmd_args_array.as_ptr();
    let env = cmd_envs_array.as_ptr();

    let args = occlum_pal_create_process_args {
        path: cmd_path.as_ptr() as *const libc::c_char,
        argv: argv as *const *const libc::c_char,
        env: env as *const *const libc::c_char,
        stdio: *stdio_raw,
        pid: &mut libos_tid as *mut i32,
    };

    let ret = unsafe { occlum_pal_create_process(&args as *const occlum_pal_create_process_args) };

    match ret {
        0 => Ok(libos_tid),
        _ => Err(ret),
    }
}

fn rust_occlum_pal_exec(occlum_process_id: i32, exit_status: &mut i32) -> Result<(), i32> {
    let args = occlum_pal_exec_args {
        pid: occlum_process_id,
        exit_value: exit_status as *mut i32,
    };

    // Disable signal handler default 8KB stack which is created in
    // https://github.com/rust-lang/rust/blob/master/library/std/src/sys/unix/stack_overflow.rs#L165
    // 8KB stack is not enough to save xsave in Intel SPR.
    // Disable sigstack here makes the handler to use the DEFAULT_STACK_SIZE
    // stack created in previous thread creation.
    // Todo: create bigger dedicated stack for signal handler.
    unsafe {
        disable_sigstack();
    }
    let result =
        panic::catch_unwind(|| unsafe { occlum_pal_exec(&args as *const occlum_pal_exec_args) });

    match result {
        Ok(ret) => {
            if ret == 0 {
                Ok(())
            } else {
                Err(ret)
            }
        }
        Err(_) => Err(-1),
    }
}

/// Send a signal to one or multiple LibOS processes
// only support SIGKILL and SIGTERM
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;

fn rust_occlum_pal_kill(pid: i32, sig: i32) -> Result<(), i32> {
    let ret = unsafe { occlum_pal_kill(pid, sig) };

    if ret == 0 {
        return Ok(());
    } else {
        return Err(ret);
    }
}
