use super::*;
use exception::*;
use fs::HostStdioFds;
use process::pid_t;
use std::ffi::{CStr, CString, OsString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use util::log::LevelFilter;
use util::mem_util::from_untrusted::*;
use util::sgx::allow_debug as sgx_allow_debug;
use sgx_tse::*;

const ENCLAVE_PATH: &'static str = ".occlum/build/lib/libocclum-libos.signed.so";

lazy_static! {
    static ref INIT_ONCE: Once = Once::new();
    static ref HAS_INIT: AtomicBool = AtomicBool::new(false);
}

#[no_mangle]
pub extern "C" fn occlum_ecall_init(log_level: *const c_char) -> i32 {
    if HAS_INIT.load(Ordering::SeqCst) == true {
        return EXIT_STATUS_INTERNAL_ERROR;
    }

    let log_level = {
        let input_log_level = match parse_log_level(log_level) {
            Err(e) => {
                eprintln!("invalid log level: {}", e.backtrace());
                return EXIT_STATUS_INTERNAL_ERROR;
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

    INIT_ONCE.call_once(|| {
        // Init the log infrastructure first so that log messages will be printed afterwards
        util::log::init(log_level);
 
        // Init MPX for SFI if MPX is available
        let report = rsgx_self_report();
        if (report.body.attributes.xfrm & SGX_XFRM_MPX != 0) {
            util::mpx_util::mpx_enable();
        }

        // Register exception handlers (support cpuid & rdtsc for now)
        register_exception_handlers();

        HAS_INIT.store(true, Ordering::SeqCst);
    });

    0
}

#[no_mangle]
pub extern "C" fn occlum_ecall_new_process(
    path_buf: *const c_char,
    argv: *const *const c_char,
    host_stdio_fds: *const HostStdioFds,
) -> i32 {
    if HAS_INIT.load(Ordering::SeqCst) == false {
        return EXIT_STATUS_INTERNAL_ERROR;
    }

    let (path, args, host_stdio_fds) = match parse_arguments(path_buf, argv, host_stdio_fds) {
        Ok(path_and_args_and_host_stdio_fds) => path_and_args_and_host_stdio_fds,
        Err(e) => {
            eprintln!("invalid arguments for LibOS: {}", e.backtrace());
            return EXIT_STATUS_INTERNAL_ERROR;
        }
    };
    let _ = backtrace::enable_backtrace(ENCLAVE_PATH, PrintFormat::Short);
    panic::catch_unwind(|| {
        backtrace::__rust_begin_short_backtrace(|| {
            match do_new_process(&path, &args, &host_stdio_fds) {
                Ok(pid_t) => pid_t as i32,
                Err(e) => {
                    eprintln!("failed to boot up LibOS: {}", e.backtrace());
                    EXIT_STATUS_INTERNAL_ERROR
                }
            }
        })
    })
    .unwrap_or(EXIT_STATUS_INTERNAL_ERROR)
}

#[no_mangle]
pub extern "C" fn occlum_ecall_exec_thread(libos_pid: i32, host_tid: i32) -> i32 {
    if HAS_INIT.load(Ordering::SeqCst) == false {
        return EXIT_STATUS_INTERNAL_ERROR;
    }

    let _ = backtrace::enable_backtrace(ENCLAVE_PATH, PrintFormat::Short);
    panic::catch_unwind(|| {
        backtrace::__rust_begin_short_backtrace(|| {
            match do_exec_thread(libos_pid as pid_t, host_tid as pid_t) {
                Ok(exit_status) => exit_status,
                Err(e) => {
                    eprintln!("failed to execute a process: {}", e.backtrace());
                    EXIT_STATUS_INTERNAL_ERROR
                }
            }
        })
    })
    .unwrap_or(EXIT_STATUS_INTERNAL_ERROR)
}

// Use -128 as a special value to indicate internal error from libos, not from
// user programs. The LibOS ensures that an user program can only return a
// value between 0 and 255 (inclusive).
const EXIT_STATUS_INTERNAL_ERROR: i32 = -128;

fn parse_log_level(level_chars: *const c_char) -> Result<LevelFilter> {
    const DEFAULT_LEVEL: LevelFilter = LevelFilter::Off;

    if level_chars.is_null() {
        return Ok(DEFAULT_LEVEL);
    }

    let level_string = {
        let level_cstring = clone_cstring_safely(level_chars)?;
        level_cstring
            .into_string()
            .map_err(|e| errno!(EINVAL, "log_level contains valid utf-8 data"))?
            .to_lowercase()
    };
    Ok(match level_string.as_str() {
        "off" => LevelFilter::Off,
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => DEFAULT_LEVEL, // Default
    })
}

fn parse_arguments(
    path_ptr: *const c_char,
    argv: *const *const c_char,
    host_stdio_fds: *const HostStdioFds,
) -> Result<(PathBuf, Vec<CString>, HostStdioFds)> {
    let path_buf = {
        let path_cstring = clone_cstring_safely(path_ptr)?;
        let path_string = path_cstring
            .into_string()
            .map_err(|e| errno!(EINVAL, "path contains valid utf-8 data"))?;
        Path::new(&path_string).to_path_buf()
    };
    let program_cstring = {
        let program_osstr = path_buf
            .file_name()
            .ok_or_else(|| errno!(EINVAL, "invalid path"))?;
        let program_str = program_osstr
            .to_str()
            .ok_or_else(|| errno!(EINVAL, "invalid path"))?;
        CString::new(program_str).map_err(|e| errno!(e))?
    };

    let mut args = clone_cstrings_safely(argv)?;
    args.insert(0, program_cstring);

    let host_stdio_fds = HostStdioFds::from_user(host_stdio_fds)?;

    Ok((path_buf, args, host_stdio_fds))
}

fn do_new_process(
    program_path: &PathBuf,
    argv: &Vec<CString>,
    host_stdio_fds: &HostStdioFds,
) -> Result<pid_t> {
    validate_program_path(program_path)?;

    let envp = &config::LIBOS_CONFIG.env;
    let file_actions = Vec::new();
    let current = &process::IDLE;
    let program_path_str = program_path.to_str().unwrap();
    let new_tid = process::do_spawn_without_exec(
        &program_path_str,
        argv,
        envp,
        &file_actions,
        host_stdio_fds,
        current,
    )?;
    Ok(new_tid)
}

fn do_exec_thread(libos_tid: pid_t, host_tid: pid_t) -> Result<i32> {
    let exit_status = process::task::exec(libos_tid, host_tid)?;

    // sync file system
    // TODO: only sync when all processes exit
    use rcore_fs::vfs::FileSystem;
    crate::fs::ROOT_INODE.fs().sync()?;

    // Only return the least significant 8 bits of the exit status
    //
    // From The Open Group Base Specifications Issue 7, 2018 edition:
    // > The shell shall recognize the entire status value retrieved for the
    // > command by the equivalent of the wait() function WEXITSTATUS macro...
    //
    // From the man page of wait() syscall:
    // > WEXITSTATUS macro returns the exit status of the child. This consists of the least
    // > significant 8 bits of the status
    let exit_status = exit_status & 0x0000_00FF_i32;
    Ok(exit_status)
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
    let is_valid_entry_point = &config::LIBOS_CONFIG
        .entry_points
        .iter()
        .any(|valid_path_prefix| target_path.starts_with(valid_path_prefix));
    if !is_valid_entry_point {
        return_errno!(EINVAL, "program path is NOT a valid entry point");
    }
    Ok(())
}
