extern crate chrono;
extern crate nix;
extern crate timer;
use crate::occlum_exec::{
    ExecComm, ExecCommResponse, GetResultRequest, GetResultResponse,
    GetResultResponse_ExecutionStatus, HealthCheckRequest, HealthCheckResponse,
    HealthCheckResponse_ServingStatus, StopRequest, StopResponse,
};
use crate::occlum_exec_grpc::OcclumExec;

use futures::stream::StreamExt;
use grpc::Metadata;
use grpc::ServerHandlerContext;
use grpc::ServerRequest;
use grpc::ServerRequestSingle;
use grpc::ServerResponseSink;
use grpc::ServerResponseUnarySink;
use sendfd::RecvWithFd;
use std::cmp;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::unix::io::RawFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Condvar, Mutex};
use std::task::Poll;
use std::thread;
use timer::{Guard, Timer};

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

#[derive(Default)]
pub struct OcclumExecImpl {
    //process_id, return value, execution status
    commands: Arc<Mutex<HashMap<u32, (Option<i32>, bool)>>>,
    execution_lock: Arc<(Mutex<bool>, Condvar)>,
    stop_timer: Arc<Mutex<Option<(Timer, Guard)>>>,
    process_id: Arc<Mutex<u32>>,
}

impl OcclumExecImpl {
    pub fn new_and_save_execution_lock(
        lock: Arc<(Mutex<bool>, Condvar)>,
        timer: (Timer, Guard),
    ) -> OcclumExecImpl {
        OcclumExecImpl {
            commands: Default::default(),
            execution_lock: lock,
            stop_timer: Arc::new(Mutex::new(Some(timer))),
            process_id: Arc::new(Mutex::new(1)),
        }
    }
}

fn reset_stop_timer(
    lock: Arc<(Mutex<bool>, Condvar)>,
    old_timer: Arc<Mutex<Option<(Timer, Guard)>>>,
    time: u32,
) {
    //New a timer to stop the server
    let timer = timer::Timer::new();
    let guard = timer.schedule_with_delay(chrono::Duration::seconds(time as i64), move || {
        if rust_occlum_pal_kill(-1, SIGKILL).is_err(){
            warn!("SIGKILL failed.")
        }
        let (execution_lock, cvar) = &*lock;
        let mut server_stopped = execution_lock.lock().unwrap();
        *server_stopped = true;
        cvar.notify_one();
    });

    let mut _old_timer = old_timer.lock().unwrap();
    *_old_timer = Some((timer, guard));
}

fn clear_stop_timer(old_timer: &Arc<Mutex<Option<(Timer, Guard)>>>) {
    let mut timer = old_timer.lock().unwrap();
    *timer = None;
}

impl OcclumExec for OcclumExecImpl {
    fn get_result(
        &self,
        _o: ServerHandlerContext,
        mut req: ServerRequestSingle<GetResultRequest>,
        resp: ServerResponseUnarySink<GetResultResponse>,
    ) -> grpc::Result<()> {
        let process_id = req.take_message().process_id;
        let commands = self.commands.clone();
        let stop_timer = self.stop_timer.clone();
        let mut commands = commands.lock().unwrap();
        let (process_status, result) = match &commands.get(&process_id) {
            None => (GetResultResponse_ExecutionStatus::UNKNOWN, -1),
            Some(&(exit_status, _)) => {
                match exit_status {
                    None => (GetResultResponse_ExecutionStatus::RUNNING, -1),
                    Some(return_value) => {
                        //Remove the process when getting the return value
                        commands.remove(&process_id);

                        if !commands.is_empty() {
                            //Clear the stop timer if some apps are running
                            clear_stop_timer(&stop_timer);
                        }

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
        if rust_occlum_pal_kill(-1, SIGTERM).is_err(){
            warn!("SIGTERM failed.");
        }
        let time = cmp::min(req.take_message().time, crate::DEFAULT_SERVER_TIMER);
        reset_stop_timer(self.execution_lock.clone(), self.stop_timer.clone(), time);
        resp.finish(StopResponse::default())
    }

    fn status_check(
        &self,
        _o: ServerHandlerContext,
        mut req: ServerRequestSingle<HealthCheckRequest>,
        resp: ServerResponseUnarySink<HealthCheckResponse>,
    ) -> grpc::Result<()> {
        //Reset the timer
        reset_stop_timer(
            self.execution_lock.clone(),
            self.stop_timer.clone(),
            crate::DEFAULT_SERVER_TIMER,
        );

        //Waits for the Occlum loaded
        let (lock, _) = &*self.execution_lock.clone();
        loop {
            let server_stopped = lock.lock().unwrap();
            if *server_stopped {
                drop(server_stopped);
                continue;
            }
            break;
        }

        //Get the process id from the request
        let process_id = req.take_message().process_id;

        match process_id {
            0 => resp.finish(HealthCheckResponse::default()),
            process_id => {
                let commands = self.commands.clone();
                let mut commands = commands.lock().unwrap();

                match commands.get_mut(&process_id) {
                    Some(_) => resp.finish(HealthCheckResponse {
                        status: HealthCheckResponse_ServingStatus::SERVING,
                        ..Default::default()
                    }),
                    _ => resp.finish(HealthCheckResponse {
                        status: HealthCheckResponse_ServingStatus::NOT_SERVING,
                        ..Default::default()
                    }),
                }
            }
        }
    }

    fn exec_command(
        &self,
        _o: ServerHandlerContext,
        mut req: ServerRequestSingle<ExecComm>,
        resp: ServerResponseUnarySink<ExecCommResponse>,
    ) -> grpc::Result<()> {
        clear_stop_timer(&self.stop_timer.clone());
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

        let gpid = self.process_id.clone();
        let mut gpid = gpid.lock().unwrap();
        let process_id: u32 = *gpid;
        *gpid += 1;
        drop(gpid);

        let _commands = self.commands.clone();
        let _execution_lock = self.execution_lock.clone();
        let _stop_timer = self.stop_timer.clone();

        let mut commands = _commands.lock().unwrap();
        commands.entry(process_id).or_insert((None, true));
        drop(commands);

        let cmd = req.command.clone();
        let args = req.parameters.into_vec().clone();
        let envs = req.enviroments.into_vec().clone();
        let client_process_id = req.process_id;

        //Run the command in a thread
        thread::spawn(move || {
            let mut exit_status = Box::new(0);
            rust_occlum_pal_exec(&cmd, &args, &envs, &stdio_fds, &mut exit_status)
                .expect("failed to execute the command");

            reset_stop_timer(_execution_lock, _stop_timer, crate::DEFAULT_SERVER_TIMER);
            let mut commands = _commands.lock().unwrap();
            *commands.get_mut(&process_id).expect("get process") = (Some(*exit_status), false);

            //Notifies the client to application stopped
            debug!(
                "process:{} finished, send signal to {}",
                process_id, client_process_id
            );

            //TODO: fix me if the client has been killed
            signal::kill(Pid::from_raw(client_process_id as i32), Signal::SIGUSR1).unwrap();
        });

        resp.finish(ExecCommResponse {
            process_id: process_id,
            ..Default::default()
        })
    }

    fn heart_beat(
        &self,
        o: ServerHandlerContext,
        req: ServerRequest<HealthCheckRequest>,
        mut resp: ServerResponseSink<HealthCheckResponse>,
    ) -> grpc::Result<()> {
        let mut req = req.into_stream();
        let commands = self.commands.clone();

        o.spawn_poll_fn(move |cx| {
            loop {
                // Wait until resp is writable
                if let Poll::Pending = resp.poll(cx)? {
                    return Poll::Pending;
                }

                match req.poll_next_unpin(cx)? {
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                    Poll::Ready(Some(note)) => {
                        let process_id = note.process_id;
                        let commands = commands.lock().unwrap();
                        let process_status = match &commands.get(&process_id) {
                            None => HealthCheckResponse_ServingStatus::UNKNOWN,
                            Some(&(exit_status, _)) => match exit_status {
                                None => HealthCheckResponse_ServingStatus::SERVING,
                                Some(_) => HealthCheckResponse_ServingStatus::NOT_SERVING,
                            },
                        };

                        resp.send_data(HealthCheckResponse {
                            status: process_status,
                            ..Default::default()
                        })
                        .unwrap();
                    }
                    Poll::Ready(None) => {
                        resp.send_trailers(Metadata::new()).expect("send");
                        return Poll::Ready(Ok(()));
                    }
                }
            }
        });
        Ok(())
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
    fn occlum_pal_create_process(args: *mut occlum_pal_create_process_args) -> i32;

    /*
     * @brief Execute the process inside the Occlum enclave
     *
     * @param args  Mandatory input. Arguments for occlum_pal_exec.
     *
     * @retval If 0, then success; otherwise, check errno for the exact error type.
     */
    fn occlum_pal_exec(args: *mut occlum_pal_exec_args) -> i32;

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

fn vec_strings_to_cchars(strings: &Vec<String>) -> Result<(Vec<*const libc::c_char>,Vec<CString>), i32> {
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
fn rust_occlum_pal_exec(
    cmd: &str,
    args: &Vec<String>,
    envs: &Vec<String>,
    stdio: &occlum_stdio_fds,
    exit_status: &mut i32,
) -> Result<(), i32> {
    let cmd_path = CString::new(cmd).expect("cmd_path: new failed");
    let (cmd_args_array, _cmd_args) = vec_strings_to_cchars(args)?;
    let (cmd_envs_array, _cmd_envs) = vec_strings_to_cchars(envs)?;

    let stdio_raw = Box::new(stdio);
    let mut libos_tid = 0;
    let create_process_args = Box::new(occlum_pal_create_process_args {
        path: cmd_path.as_ptr() as *const libc::c_char,
        argv: Box::into_raw(cmd_args_array.into_boxed_slice()) as *const *const libc::c_char,
        env: Box::into_raw(cmd_envs_array.into_boxed_slice()) as *const *const libc::c_char,
        stdio: *stdio_raw,
        pid: &mut libos_tid as *mut i32,
    });

    let ret = unsafe{occlum_pal_create_process(Box::into_raw(create_process_args))};
    if ret != 0 {
        return Err(ret);
    }

    let exec_args = Box::new(occlum_pal_exec_args {
        pid: libos_tid,
        exit_value: exit_status as *mut i32,
    });

    let ret = unsafe {occlum_pal_exec(Box::into_raw(exec_args))};

    match ret {
        0 => Ok(()),
        _ => Err(ret),
    }
}

/// Send a signal to one or multiple LibOS processes 
// only support SIGKILL and SIGTERM 
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;

fn rust_occlum_pal_kill(pid: i32, sig: i32) -> Result<i32, i32> {
    let ret = unsafe { occlum_pal_kill(pid, sig) };

    if ret == 0 {
        return Ok(0);
    } else {
        return Err(ret);
    }
}
