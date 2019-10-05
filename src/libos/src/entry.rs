use super::*;
use exception::*;
use process::pid_t;
use std::ffi::{CStr, CString, OsString};
use std::path::Path;
use util::mem_util::from_untrusted::*;

const ENCLAVE_PATH: &'static str = ".occlum/build/lib/libocclum.signed.so";

#[no_mangle]
pub extern "C" fn libos_boot(path_buf: *const c_char, argv: *const *const c_char) -> i32 {
    // Init the log infrastructure first so that log messages will be printed afterwards
    util::log::init();

    let (path, args) = match parse_arguments(path_buf, argv) {
        Ok(path_and_args) => path_and_args,
        Err(e) => {
            error!("invalid arguments for LibOS: {}", e.backtrace());
            return EXIT_STATUS_INTERNAL_ERROR;
        }
    };

    // register exception handlers (support cpuid & rdtsc for now)
    register_exception_handlers();

    let _ = backtrace::enable_backtrace(ENCLAVE_PATH, PrintFormat::Short);
    panic::catch_unwind(|| {
        backtrace::__rust_begin_short_backtrace(|| match do_boot(&path, &args) {
            Ok(()) => 0,
            Err(e) => {
                error!("failed to boot up LibOS: {}", e.backtrace());
                EXIT_STATUS_INTERNAL_ERROR
            }
        })
    })
    .unwrap_or(EXIT_STATUS_INTERNAL_ERROR)
}

#[no_mangle]
pub extern "C" fn libos_run(host_tid: i32) -> i32 {
    let _ = backtrace::enable_backtrace(ENCLAVE_PATH, PrintFormat::Short);
    panic::catch_unwind(|| {
        backtrace::__rust_begin_short_backtrace(|| match do_run(host_tid as pid_t) {
            Ok(exit_status) => exit_status,
            Err(e) => {
                error!("failed to execute a process: {}", e.backtrace());
                EXIT_STATUS_INTERNAL_ERROR
            }
        })
    })
    .unwrap_or(EXIT_STATUS_INTERNAL_ERROR)
}

#[no_mangle]
pub extern "C" fn dummy_ecall() -> i32 {
    0
}
// Use 127 as a special value to indicate internal error from libos, not from
// user programs, although it is completely ok for a user program to return 127.
const EXIT_STATUS_INTERNAL_ERROR: i32 = 127;

fn parse_arguments(
    path_buf: *const c_char,
    argv: *const *const c_char,
) -> Result<(String, Vec<CString>)> {
    let path_string = {
        let path_cstring = clone_cstring_safely(path_buf)?;
        path_cstring.to_string_lossy().into_owned()
    };
    let program_cstring = {
        let program_osstr = Path::new(&path_string)
            .file_name()
            .ok_or_else(|| errno!(EINVAL, "invalid path"))?;
        let program_str = program_osstr
            .to_str()
            .ok_or_else(|| errno!(EINVAL, "invalid path"))?;
        CString::new(program_str).map_err(|e| errno!(e))?
    };

    let mut args = clone_cstrings_safely(argv)?;
    args.insert(0, program_cstring);
    Ok((path_string, args))
}

// TODO: make sure do_boot can only be called once
fn do_boot(path_str: &str, argv: &Vec<CString>) -> Result<()> {
    //    info!("boot: path: {:?}, argv: {:?}", path_str, argv);
    util::mpx_util::mpx_enable()?;

    let envp = &config::LIBOS_CONFIG.env;
    let file_actions = Vec::new();
    let parent = &process::IDLE_PROCESS;
    process::do_spawn(&path_str, argv, envp, &file_actions, parent)?;

    Ok(())
}

// TODO: make sure do_run() cannot be called after do_boot()
fn do_run(host_tid: pid_t) -> Result<i32> {
    let exit_status = process::run_task(host_tid)?;

    // sync file system
    // TODO: only sync when all processes exit
    use rcore_fs::vfs::FileSystem;
    crate::fs::ROOT_INODE.fs().sync()?;

    Ok(exit_status)
}
