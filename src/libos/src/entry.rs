use std::ffi::{CStr, CString, OsString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

use super::*;
use crate::exception::*;
use crate::fs::{HostStdioFds, ROOT_FS, SEFS_MANAGER};
use crate::interrupt;
use crate::io_uring::ENABLE_URING;
use crate::process::idle_reap_zombie_children;
use crate::process::{ProcessFilter, SpawnAttr};
use crate::signal::SigNum;
use crate::time::init;
use crate::util::host_file_util::{host_file_buffer, parse_host_file, write_host_file, HostFile};
use crate::util::log::LevelFilter;
use crate::util::mem_util::from_untrusted::*;
use crate::util::sgx::allow_debug as sgx_allow_debug;
use crate::vm::USER_SPACE_VM_MANAGER;
use sgx_tse::*;

pub static mut INSTANCE_DIR: String = String::new();
static mut ENCLAVE_PATH: String = String::new();

/// Note about memory ordering:
/// HAS_INIT need to synchronize the relevant resources in interrupt::init().
/// The read operation of HAS_INIT needs to see the change of the resources.
/// Just `Acquire` or `Release` needs to be used to make all the change of the
/// wakers visible to us.
lazy_static! {
    static ref INIT_ONCE: Once = Once::new();
    static ref HAS_INIT: AtomicBool = AtomicBool::new(false);
    pub static ref ENTRY_POINTS: RwLock<Vec<PathBuf>> = RwLock::new(
        config::LIBOS_CONFIG
            .get_app_config("init")
            .unwrap()
            .entry_points
            .clone()
    );
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

#[no_mangle]
pub extern "C" fn occlum_ecall_init(
    log_level: *const c_char,
    instance_dir: *const c_char,
    file_buffer: *const host_file_buffer,
) -> i32 {
    if HAS_INIT.load(Ordering::Acquire) == true {
        return ecall_errno!(EEXIST);
    }

    assert!(!instance_dir.is_null());

    let log_level = match parse_log_level(log_level) {
        Err(e) => {
            eprintln!("invalid log level: {}", e.backtrace());
            return ecall_errno!(EINVAL);
        }
        Ok(log_level) => log_level,
    };

    INIT_ONCE.call_once(|| {
        // Init the log infrastructure first so that log messages will be printed afterwards
        // The log level may be set to off later if disable_log is true in user configuration
        util::log::init(log_level);

        let report = rsgx_self_report();
        // Init MPX for SFI if MPX is available
        if (report.body.attributes.xfrm & SGX_XFRM_MPX != 0) {
            util::mpx_util::mpx_enable();
        }
        // Init PKU for isolating LibOS form user-apps if PKU is available
        // Occlum only turns on `pku` feature in HW mode
        #[cfg(feature = "pku")]
        {
            if (report.body.attributes.xfrm & SGX_XFRM_PKRU != 0) {
                crate::util::pku_util::try_set_pku_enabled();
            }
        }

        unsafe {
            let dir_str: &str = CStr::from_ptr(instance_dir).to_str().unwrap();
            INSTANCE_DIR.push_str(dir_str);
            ENCLAVE_PATH.push_str(&INSTANCE_DIR);
            ENCLAVE_PATH.push_str("/build/lib/libocclum-libos.signed.so");
        }

        interrupt::init();

        // Init vdso and boot up time stamp here.
        time::init();

        vm::init_user_space();

        if ENABLE_URING.load(Ordering::Relaxed) {
            crate::io_uring::MULTITON.poll_completions();
        }

        // Register exception handlers (support cpuid & rdtsc for now)
        register_exception_handlers();

        HAS_INIT.store(true, Ordering::Release);
        // Enable global backtrace
        unsafe { backtrace::enable_backtrace(&ENCLAVE_PATH, PrintFormat::Short) };

        // Add hook for allocation error
        std::alloc::set_alloc_error_hook(oom_handle);
    });

    panic::catch_unwind(|| {
        backtrace::__rust_begin_short_backtrace(|| {
            // Ignore the error when parsing host files
            let _ = parse_host_files(file_buffer);
            0 as i32
        })
    })
    .unwrap_or(ecall_errno!(EFAULT))
}

// hook for memory allocation error
fn oom_handle(layout: std::alloc::Layout) {
    panic!("memory allocation of {} bytes failed\n", layout.size());
}

#[no_mangle]
pub extern "C" fn occlum_ecall_new_process(
    path_buf: *const c_char,
    argv: *const *const c_char,
    env: *const *const c_char,
    host_stdio_fds: *const HostStdioFds,
) -> i32 {
    if HAS_INIT.load(Ordering::Acquire) == false {
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

    panic::catch_unwind(|| {
        backtrace::__rust_begin_short_backtrace(|| {
            match do_new_process(&path, &args, env, &host_stdio_fds) {
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
pub extern "C" fn occlum_ecall_exec_thread(libos_pid: i32, host_tid: i32) -> i32 {
    if HAS_INIT.load(Ordering::Acquire) == false {
        return ecall_errno!(EAGAIN);
    }

    panic::catch_unwind(|| {
        backtrace::__rust_begin_short_backtrace(|| {
            match do_exec_thread(libos_pid as pid_t, host_tid as pid_t) {
                Ok(exit_status) => exit_status,
                Err(e) => {
                    eprintln!("failed to execute a process: {}", e.backtrace());
                    ecall_errno!(e.errno())
                }
            }
        })
    })
    .unwrap_or(ecall_errno!(EFAULT))
}

#[no_mangle]
pub extern "C" fn occlum_ecall_kill(pid: i32, sig: i32) -> i32 {
    if HAS_INIT.load(Ordering::Acquire) == false {
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

#[no_mangle]
pub extern "C" fn occlum_ecall_broadcast_interrupts() -> i32 {
    if HAS_INIT.load(Ordering::Acquire) == false {
        return ecall_errno!(EAGAIN);
    }

    panic::catch_unwind(|| {
        backtrace::__rust_begin_short_backtrace(|| match interrupt::broadcast_interrupts() {
            Ok(count) => count as i32,
            Err(e) => {
                eprintln!("failed to broadcast interrupts: {}", e.backtrace());
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

    let new_tid = process::do_spawn_without_exec(
        &program_path_str,
        argv,
        &env_concat,
        &file_actions,
        Some(spawn_attribute),
        host_stdio_fds,
        current,
    )?;
    Ok(new_tid)
}

fn do_exec_thread(libos_tid: pid_t, host_tid: pid_t) -> Result<i32> {
    let status = process::task::exec(libos_tid, host_tid)?;

    // Idle process should reap all zombie children
    idle_reap_zombie_children()?;

    // We sync all mounted SEFSes before the thread exits
    // to flush cached data and free memory.
    let _rootfs = ROOT_FS.read().unwrap();
    SEFS_MANAGER.update_then_sync_all()?;

    // Not to be confused with the return value of a main function.
    // The exact meaning of status is described in wait(2) man page.
    Ok(status)
}

fn validate_program_path(target_path: &PathBuf) -> Result<()> {
    if !target_path.is_absolute() {
        return_errno!(EINVAL, "program path must be absolute");
    }

    // Forbid paths like /bin/../root, which may circument our prefix-based path matching
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

    let env_listed = &config::LIBOS_CONFIG.env.untrusted;
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

// Parse host files
fn parse_host_files(file_buffer: *const host_file_buffer) -> Result<i32> {
    let resolv_conf_ptr = unsafe { (*file_buffer).resolv_conf_buf };
    match parse_host_file(HostFile::ResolvConf, resolv_conf_ptr) {
        Err(e) => {
            warn!("failed to parse /etc/resolv.conf: {}", e.backtrace());
        }
        Ok(resolv_conf_str) => {
            *RESOLV_CONF_STR.write().unwrap() = Some(resolv_conf_str);
            if let Err(e) = write_host_file(HostFile::ResolvConf) {
                warn!("failed to write /etc/resolv.conf: {}", e.backtrace());
            }
        }
    }

    let hostname_ptr = unsafe { (*file_buffer).hostname_buf };
    match parse_host_file(HostFile::HostName, hostname_ptr) {
        Err(e) => {
            warn!("failed to parse /etc/hostname: {}", e.backtrace());
        }
        Ok(hostname_str) => {
            misc::init_nodename(&hostname_str);
            *HOSTNAME_STR.write().unwrap() = Some(hostname_str);
            if let Err(e) = write_host_file(HostFile::HostName) {
                warn!("failed to write /etc/hostname: {}", e.backtrace());
            }
        }
    }

    let hosts_ptr = unsafe { (*file_buffer).hosts_buf };
    match parse_host_file(HostFile::Hosts, hosts_ptr) {
        Err(e) => {
            warn!("failed to parse /etc/hosts: {}", e.backtrace());
        }
        Ok(hosts_str) => {
            *HOSTS_STR.write().unwrap() = Some(hosts_str);
            if let Err(e) = write_host_file(HostFile::Hosts) {
                warn!("failed to write /etc/hosts: {}", e.backtrace());
            }
        }
    }

    Ok(0)
}
