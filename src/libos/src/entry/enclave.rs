use std::backtrace::{self, PrintFormat};
use std::ffi::{CStr, CString, OsString};
use std::panic::{self};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Once;

use sgx_tse::*;

use crate::fs::HostStdioFds;
use crate::misc;
use crate::prelude::*;
use crate::process::{self, table, ProcessFilter, SpawnAttr};
use crate::signal::SigNum;
use crate::time::up_time::init;
use crate::util::host_file_util::{host_file_buffer, parse_host_file, write_host_file, HostFile};
use crate::util::log::LevelFilter;
use crate::util::mem_util::from_untrusted::*;
use crate::util::sgx::allow_debug as sgx_allow_debug;

pub static mut INSTANCE_DIR: String = String::new();
static mut ENCLAVE_PATH: String = String::new();

// TCS used by Occlum kernel.
// For "occlum run", we need 3 kernel thread:
// * main thread to do occlum_ecall_new_process
// * occlum timer thread to do occlum_ecall_timer_thread_create
// * polling thread for io_uring
//
// For "occlum exec", we need 2 extra kernel thread:
// * init process will call occlum_ecall_new_process in another thread different from the main thread
// * "occlum stop" will create a new thread to do occlum_ecall_kill
const OCCLUM_KERNEL_TCS_NUM: u32 = 5;

lazy_static! {
    static ref INIT_ONCE: Once = Once::new();
    static ref HAS_INIT: AtomicBool = AtomicBool::new(false);
    pub static ref ENTRY_POINTS: RwLock<Vec<PathBuf>> = RwLock::new(
        crate::config::LIBOS_CONFIG
            .get_app_config("init")
            .unwrap()
            .entry_points
            .clone()
    );
    static ref HOST_FILE_INIT_ONCE: Once = Once::new();
    pub static ref RESOLV_CONF_STR: RwLock<Option<String>> = RwLock::new(None);
    pub static ref HOSTNAME_STR: RwLock<Option<String>> = RwLock::new(None);
    pub static ref HOSTS_STR: RwLock<Option<String>> = RwLock::new(None);
}

macro_rules! ecall_errno {
    ($errno:expr) => {{
        let errno: Errno = $errno;
        -(errno as i32)
    }};
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct occlum_pal_vcpu_data {
    user_space_mark: u32,
}

#[no_mangle]
pub extern "C" fn occlum_ecall_init(
    log_level: *const c_char,
    instance_dir: *const c_char,
    file_buffer: *const host_file_buffer,
    num_vcpus: u32,
) -> i32 {
    if HAS_INIT.load(Ordering::SeqCst) == true {
        return ecall_errno!(EEXIST);
    }

    assert!(!instance_dir.is_null());

    let log_level = {
        let input_log_level = match parse_log_level(log_level) {
            Err(e) => {
                eprintln!("invalid log level: {}", e.backtrace());
                return ecall_errno!(EINVAL);
            }
            Ok(log_level) => log_level,
        };
        // Use the input log level if and only if the enclave allows debug
        if sgx_allow_debug() {
            input_log_level
        } else {
            LevelFilter::Off
        }
    };

    let max_tcs_num = std::enclave::get_tcs_max_num();
    info!(
        "num_vcpus = {:?}, max_tcs_num = {:?}",
        num_vcpus, max_tcs_num
    );
    assert!(num_vcpus > 0 && num_vcpus <= 1024);
    if max_tcs_num < num_vcpus + OCCLUM_KERNEL_TCS_NUM {
        eprintln!(
            "Can't create {:?} vcpus. Please enlarge the \"num_of_cpus\" in Occlum.yaml\n",
            num_vcpus
        );
        return ecall_errno!(EINVAL);
    }

    INIT_ONCE.call_once(|| {
        // Init the log infrastructure first so that log messages will be printed afterwards
        crate::util::log::init(log_level);

        let report = rsgx_self_report();
        // Init MPX for SFI if MPX is available
        if (report.body.attributes.xfrm & SGX_XFRM_MPX != 0) {
            crate::util::mpx_util::mpx_enable();
        }
        // Init PKU for isolating LibOS form user-apps if PKU is available
        // Occlum only turns on `pku` feature in HW mode
        #[cfg(feature = "pku")]
        {
            if (report.body.attributes.xfrm & SGX_XFRM_PKRU != 0) {
                crate::util::pku_util::try_set_pku_enabled();
            }
        }

        // Register exception handlers (support cpuid & rdtsc for now)
        super::exception::register_exception_handlers();

        unsafe {
            let dir_str: &str = CStr::from_ptr(instance_dir).to_str().unwrap();
            INSTANCE_DIR.push_str(dir_str);
            ENCLAVE_PATH.push_str(&INSTANCE_DIR);
            ENCLAVE_PATH.push_str("/build/lib/libocclum-libos.signed.so");
        }

        super::interrupt::init();

        async_rt::vcpu::set_total(num_vcpus);

        std::thread::spawn(move || {
            let io_uring = &crate::io_uring::SINGLETON;
            loop {
                let min_complete = 1;
                let polling_retries = 10000;
                io_uring.poll_completions(min_complete, polling_retries);
            }
        });

        // Init boot up time stamp here.
        crate::time::up_time::init();

        // Init untrusted unix sockets
        crate::net::untrusted_unix_socks_init();

        // Enable global backtrace
        unsafe { std::backtrace::enable_backtrace(&ENCLAVE_PATH, PrintFormat::Full) };

        // Start load balancer
        async_rt::executor::start_load_balancer();

        // Parse host file and store in memory
        let resolv_conf_ptr = unsafe { (*file_buffer).resolv_conf_buf };
        match parse_host_file(HostFile::ResolvConf, resolv_conf_ptr) {
            Err(e) => {
                error!("failed to parse host /etc/resolv.conf: {}", e.backtrace());
            }
            Ok(resolv_conf_str) => {
                *RESOLV_CONF_STR.write().unwrap() = Some(resolv_conf_str);
            }
        }

        let hostname_ptr = unsafe { (*file_buffer).hostname_buf };
        match parse_host_file(HostFile::HostName, hostname_ptr) {
            Err(e) => {
                error!("failed to parse host /etc/hostname: {}", e.backtrace());
            }
            Ok(hostname_str) => {
                misc::init_nodename(&hostname_str);
                *HOSTNAME_STR.write().unwrap() = Some(hostname_str);
            }
        }

        let hosts_ptr = unsafe { (*file_buffer).hosts_buf };
        match parse_host_file(HostFile::Hosts, hosts_ptr) {
            Err(e) => {
                error!("failed to parse host /etc/hosts: {}", e.backtrace());
            }
            Ok(hosts_str) => {
                *HOSTS_STR.write().unwrap() = Some(hosts_str);
            }
        }

        HAS_INIT.store(true, Ordering::SeqCst);
    });

    0
}

#[no_mangle]
pub extern "C" fn occlum_ecall_new_process(
    path_buf: *const c_char,
    argv: *const *const c_char,
    env: *const *const c_char,
    host_stdio_fds: *const HostStdioFds,
    wake_host: *mut i32,
) -> i32 {
    if HAS_INIT.load(Ordering::SeqCst) == false {
        return ecall_errno!(EAGAIN);
    }

    let (path, args, env, host_stdio_fds) =
        match parse_arguments(path_buf, argv, env, host_stdio_fds) {
            Ok(all_parsed_args) => all_parsed_args,
            Err(e) => {
                eprintln!("invalid arguments for LibOS: {}", e.backtrace());
                return ecall_errno!(e.errno());
            }
        };

    // Convert *mut i32 to usize to satisfy the requirement of Send/Sync
    let wake_host_addr = wake_host as usize;
    panic::catch_unwind(|| {
        backtrace::__rust_begin_short_backtrace(|| {
            let wake_host = wake_host_addr as *mut i32;
            match do_new_process(&path, &args, env, &host_stdio_fds, wake_host) {
                Ok(pid_t) => pid_t as i32,
                Err(e) => {
                    eprintln!("failed to boot up LibOS: {}", e.backtrace());
                    ecall_errno!(e.errno())
                }
            }
        })
    })
    .unwrap_or(ecall_errno!(EFAULT))
}

#[no_mangle]
pub extern "C" fn occlum_ecall_run_vcpu(pal_data_ptr: *const occlum_pal_vcpu_data) -> i32 {
    if HAS_INIT.load(Ordering::SeqCst) == false {
        return ecall_errno!(EAGAIN);
    }

    assert!(!pal_data_ptr.is_null());
    assert!(check_ptr(pal_data_ptr).is_ok()); // Make sure the ptr is outside the enclave
    set_pal_data_addr(pal_data_ptr);

    let running_vcpu_num = async_rt::executor::run_tasks();
    if running_vcpu_num == 0 {
        // It is the last vcpu for the executor. We can perform some check to make sure there is no resource leakage
        assert!(
            table::get_all_pgrp().len() == 0
                && table::get_all_processes().len() == 0
                && table::get_all_threads().len() == 0
        );
    }

    0
}

#[no_mangle]
pub extern "C" fn occlum_ecall_timer_thread_create() -> i32 {
    if HAS_INIT.load(Ordering::SeqCst) == false {
        return ecall_errno!(EAGAIN);
    }

    async_rt::time::run_timer_wheel_thread();
    0
}

#[no_mangle]
pub extern "C" fn occlum_ecall_shutdown_vcpus() -> i32 {
    if HAS_INIT.load(Ordering::SeqCst) == false {
        return ecall_errno!(EAGAIN);
    }

    // Send SIGKILL to all existing process
    use crate::signal::SIGKILL;
    crate::signal::do_kill_from_outside_enclave(ProcessFilter::WithAnyPid, SIGKILL);

    table::wait_all_process_exit();

    async_rt::task::block_on(async {
        // Flush the async rootfs
        crate::fs::rootfs().await.sync().await.unwrap();

        // Free user space VM
        crate::vm::free_user_space().await;
    });

    // TODO: stop all the kernel threads/tasks
    async_rt::executor::shutdown();
    0
}

#[no_mangle]
pub extern "C" fn occlum_ecall_init_host_file() -> i32 {
    if HAS_INIT.load(Ordering::SeqCst) == false {
        return ecall_errno!(EAGAIN);
    }

    HOST_FILE_INIT_ONCE.call_once(|| {
        async_rt::task::block_on(unsafe {
            super::thread::mark_send::mark_send(async {
                if let Err(e) = write_host_file(HostFile::ResolvConf).await {
                    error!("failed to init /etc/resolv.conf: {}", e.backtrace());
                }
                if let Err(e) = write_host_file(HostFile::HostName).await {
                    error!("failed to init /etc/hostname: {}", e.backtrace());
                }
                if let Err(e) = write_host_file(HostFile::Hosts).await {
                    error!("failed to init /etc/hosts: {}", e.backtrace());
                }
            })
        });
    });

    0
}

#[no_mangle]
pub extern "C" fn occlum_ecall_kill(pid: i32, sig: i32) -> i32 {
    if HAS_INIT.load(Ordering::SeqCst) == false {
        return ecall_errno!(EAGAIN);
    }

    panic::catch_unwind(|| {
        backtrace::__rust_begin_short_backtrace(|| match do_kill(pid, sig) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("failed to kill: {}", e.backtrace());
                ecall_errno!(e.errno())
            }
        })
    })
    .unwrap_or(ecall_errno!(EFAULT))
}

fn parse_log_level(level_chars: *const c_char) -> Result<LevelFilter> {
    const DEFAULT_LEVEL: LevelFilter = LevelFilter::Off;

    if level_chars.is_null() {
        return Ok(DEFAULT_LEVEL);
    }

    let level_string = {
        // level_chars has been guaranteed to be inside enclave
        // and null terminated by ECall
        let level_cstring = CString::from(unsafe { CStr::from_ptr(level_chars) });
        level_cstring
            .into_string()
            .map_err(|e| errno!(EINVAL, "log_level contains valid utf-8 data"))?
            .to_lowercase()
    };
    Ok(match level_string.as_str() {
        "off" => LevelFilter::Off,
        "panic" | "fatal" | "error" => LevelFilter::Error,
        "warning" | "warn" => LevelFilter::Warn, // Panic, fatal and warning are log levels defined in OCI (Open Container Initiative)
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => DEFAULT_LEVEL, // Default
    })
}

fn parse_arguments(
    path_ptr: *const c_char,
    argv: *const *const c_char,
    env: *const *const c_char,
    host_stdio_fds: *const HostStdioFds,
) -> Result<(PathBuf, Vec<CString>, Vec<CString>, HostStdioFds)> {
    let path_buf = {
        if path_ptr.is_null() {
            return_errno!(EINVAL, "empty path");
        }
        // path_ptr has been guaranteed to be inside enclave
        // and null terminated by ECall
        let path_cstring = CString::from(unsafe { CStr::from_ptr(path_ptr) });

        let path_string = path_cstring
            .into_string()
            .map_err(|e| errno!(EINVAL, "path contains valid utf-8 data"))?;
        Path::new(&path_string).to_path_buf()
    };

    let mut args = clone_cstrings_safely(argv)?;

    let env_merged = merge_env(env)?;
    trace!(
        "env_merged = {:?}  (default env and untrusted env)",
        env_merged
    );

    let host_stdio_fds = HostStdioFds::from_user(host_stdio_fds)?;

    Ok((path_buf, args, env_merged, host_stdio_fds))
}

fn do_new_process(
    program_path: &PathBuf,
    argv: &Vec<CString>,
    env_concat: Vec<CString>,
    host_stdio_fds: &HostStdioFds,
    wake_host: *mut i32,
) -> Result<pid_t> {
    validate_program_path(program_path)?;

    let file_actions = Vec::new();
    let current = &process::IDLE;
    let program_path_str = program_path.to_str().unwrap();

    // Called from occlum_ecall_new_process, give it an identical process group.
    // So that "occlum run/exec" process will have its own process group.
    let spawn_attribute = {
        let mut attribute = SpawnAttr::default();
        attribute.process_group = Some(0);
        attribute
    };

    // Clone some arguments for calling async
    let program_path_str = program_path_str.to_owned();
    let argv = argv.to_owned();
    let host_stdio_fds = host_stdio_fds.to_owned();

    let new_tid = async_rt::task::block_on(unsafe {
        super::thread::mark_send::mark_send(async move {
            process::do_spawn_root(
                &program_path_str,
                &argv,
                &env_concat,
                &file_actions,
                Some(spawn_attribute),
                &host_stdio_fds,
                wake_host,
                current,
            )
            .await
        })
    })?;

    Ok(new_tid)
}

fn validate_program_path(target_path: &PathBuf) -> Result<()> {
    if !target_path.is_absolute() {
        return_errno!(EINVAL, "program path must be absolute");
    }

    // Forbid paths like /bin/../root, which may circumvent our prefix-based path matching
    let has_parent_component = {
        target_path
            .components()
            .any(|component| component == std::path::Component::ParentDir)
    };
    if has_parent_component {
        return_errno!(
            EINVAL,
            "program path cannot contain any parent component (i.e., \"..\")"
        );
    }

    // Check whether the prefix of the program path matches one of the entry points
    let is_valid_entry_point = &ENTRY_POINTS
        .read()
        .unwrap()
        .iter()
        .any(|valid_path_prefix| target_path.starts_with(valid_path_prefix));
    if !is_valid_entry_point {
        return_errno!(EACCES, "program path is NOT a valid entry point");
    }
    Ok(())
}

fn do_kill(pid: i32, sig: i32) -> Result<()> {
    let filter = if pid > 0 {
        ProcessFilter::WithPid(pid as pid_t)
    } else if pid == -1 {
        ProcessFilter::WithAnyPid
    } else if pid < 0 {
        return_errno!(EINVAL, "Invalid pid");
    } else {
        // pid == 0
        return_errno!(EPERM, "Process 0 cannot be killed");
    };
    let signum = {
        if sig < 0 {
            return_errno!(EINVAL, "invalid arguments");
        }
        SigNum::from_u8(sig as u8)?
    };
    crate::signal::do_kill_from_outside_enclave(filter, signum)
}

fn merge_env(env: *const *const c_char) -> Result<Vec<CString>> {
    #[derive(Debug)]
    struct EnvDefaultInner {
        content: Vec<CString>,
        helper: HashMap<String, usize>, // Env key: index of content
    }

    let env_listed = &crate::config::LIBOS_CONFIG.env.untrusted;
    let mut env_checked: Vec<CString> = Vec::new();
    let mut env_default = EnvDefaultInner {
        content: Vec::new(),
        helper: HashMap::new(),
    };

    let config_env_trusted = crate::config::TRUSTED_ENVS.read().unwrap();

    // Use inner struct to parse env default
    for (idx, val) in config_env_trusted.iter().enumerate() {
        env_default.content.push(CString::new(val.clone())?);
        let kv: Vec<&str> = val.to_str().unwrap().splitn(2, '=').collect(); // only split the first "="
        env_default.helper.insert(kv[0].to_string(), idx);
    }

    // Filter out env which are not listed in Occlum.json env untrusted section
    // and record the index of env default element if it is overridden
    let mut remove_idx: Vec<usize> = Vec::new();
    if (!env.is_null()) {
        let env_untrusted = clone_cstrings_safely(env)?;
        for iter in env_untrusted.iter() {
            let env_kv: Vec<&str> = iter.to_str().unwrap().splitn(2, '=').collect();
            if env_listed.contains(env_kv[0]) {
                env_checked.push(iter.clone());
                if let Some(idx) = env_default.helper.get(env_kv[0]) {
                    remove_idx.push(*idx);
                }
            }
        }
    }

    // Only keep items in env_default if they are not overridden by untrusted envs
    let mut env_keep: Vec<CString> = Vec::new();
    for (idx, val) in env_default.content.iter().enumerate() {
        if !remove_idx.contains(&idx) {
            env_keep.push(CString::new(val.clone())?);
        }
    }

    trace!("env_checked from env untrusted: {:?}", env_checked);
    Ok([env_keep, env_checked].concat())
}

pub(crate) fn set_pal_data_addr(pal_data_ptr: *const occlum_pal_vcpu_data) {
    let pal_data_addr = pal_data_ptr as usize;
    PAL_DATA.store(pal_data_addr, Ordering::Relaxed);
}

pub fn set_user_space_mark(mark: u32) {
    let pal_data_addr = PAL_DATA.load(Ordering::Relaxed);
    let pal_data_ptr = unsafe { &mut *(pal_data_addr as *mut occlum_pal_vcpu_data) };
    let mut mark = mark;

    // Add 1 to switch count every time before entering user space.
    // Reset it to 0 after switching back.
    // The logic here is if the switch count keeps no change, one task in this
    // VCPU is somehow blocked in userspace.
    if mark != 0 {
        mark = CONTEXT_SWITCH_CNT.fetch_add(1, Ordering::Relaxed);
    }

    (*pal_data_ptr).user_space_mark = mark;
}

#[thread_local]
static PAL_DATA: AtomicUsize = AtomicUsize::new(0);
static CONTEXT_SWITCH_CNT: AtomicU32 = AtomicU32::new(0);
