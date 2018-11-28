use prelude::*;
use {std, file, file_table, fs, process};
use std::ffi::{CStr, CString};
// Use the internal syscall wrappers from sgx_tstd
//use std::libc_fs as fs;
//use std::libc_io as io;

fn check_ptr_from_user<T>(user_ptr: *const T) -> Result<*const T, Error> {
    Ok(user_ptr)
}

fn check_mut_ptr_from_user<T>(user_ptr: *mut T) -> Result<*mut T, Error> {
    Ok(user_ptr)
}

fn clone_string_from_user_safely(user_ptr: *const c_char)
    -> Result<String, Error>
{
    let user_ptr = check_ptr_from_user(user_ptr)?;
    let string = unsafe {
        CStr::from_ptr(user_ptr).to_string_lossy().into_owned()
    };
    Ok(string)
}

fn clone_cstrings_from_user_safely(user_ptr: *const *const c_char)
    -> Result<Vec<CString>, Error>
{
    let cstrings = Vec::new();
    Ok(cstrings)
}


#[no_mangle]
pub extern "C" fn occlum_open(path_buf: * const c_char, flags: c_int, mode: c_int) -> c_int {
    let path = unsafe {
        CStr::from_ptr(path_buf as * const i8).to_string_lossy().into_owned()
    };
    match fs::do_open(&path, flags as u32, mode as u32) {
        Ok(fd) => {
            fd as c_int
        },
        Err(e) => {
            e.errno.as_retval()
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_close(fd: c_int) -> c_int {
    match fs::do_close(fd as file_table::FileDesc) {
        Ok(()) => {
            0
        },
        Err(e) => {
            e.errno.as_retval()
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_read(fd: c_int, buf: * mut c_void, size: size_t) -> ssize_t {
    let buf = unsafe {
        std::slice::from_raw_parts_mut(buf as *mut u8, size as usize)
    };
    match fs::do_read(fd as file_table::FileDesc, buf) {
        Ok(read_len) => {
            read_len as ssize_t
        },
        Err(e) => {
            e.errno.as_retval() as ssize_t
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_write(fd: c_int, buf: * const c_void, size: size_t) -> ssize_t {
/*    let str_from_c = unsafe {
        CStr::from_ptr(buf as * const i8).to_string_lossy().into_owned()
    };
    println!("occlum_write: {}", str_from_c);
    size as ssize_t
*/
    let buf = unsafe {
        std::slice::from_raw_parts(buf as *const u8, size as usize)
    };
    match fs::do_write(fd as file_table::FileDesc, buf) {
        Ok(write_len) => {
            write_len as ssize_t
        },
        Err(e) => {
            e.errno.as_retval() as ssize_t
        }
    }
}

#[no_mangle]
pub extern "C" fn occlum_getpid() -> c_uint
{
    process::do_getpid()
}

#[no_mangle]
pub extern "C" fn occlum_exit(status: i32)
{
    process::do_exit(status);
}

#[no_mangle]
pub extern "C" fn occlum_unknown(num: u32)
{
    println!("[WARNING] Unknown syscall (num = {})", num);
}

fn do_spawn(child_pid_ptr: *mut c_uint,
            path: *const c_char,
            argv: *const *const c_char,
            envp: *const *const c_char)
    -> Result<(), Error>
{
    let child_pid_ptr = check_mut_ptr_from_user(child_pid_ptr)?;
    let path = clone_string_from_user_safely(path)?;
    let argv = clone_cstrings_from_user_safely(argv)?;
    let envp = clone_cstrings_from_user_safely(envp)?;

    let child_pid = process::do_spawn(&path, &argv, &envp)?;

    unsafe { *child_pid_ptr = child_pid };
    Ok(())
}

#[no_mangle]
pub extern "C" fn occlum_spawn(
    child_pid: *mut c_uint, path: *const c_char,
    argv: *const *const c_char, envp: *const *const c_char) -> c_int
{
    match do_spawn(child_pid, path, argv, envp) {
        Ok(()) => 0,
        Err(e) => { e.errno.as_retval() }
    }
}

#[no_mangle]
pub extern "C" fn occlum_wait4(child_pid: c_int, _exit_code: *mut c_int,
    options: c_int/*, rusage: *mut Rusage*/) -> c_int
{
    match process::do_wait4(child_pid as u32) {
        Ok(exit_code) => unsafe {
            *_exit_code = exit_code;
            0
        }
        Err(e) => {
            e.errno.as_retval()
        }
    }
}
