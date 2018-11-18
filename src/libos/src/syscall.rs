use prelude::*;
use {std, file, file_table, fs, process};
use std::ffi::CStr; // a borrowed C string
// Use the internal syscall wrappers from sgx_tstd
//use std::libc_fs as fs;
//use std::libc_io as io;

// TODO: check all pointer passed from user belongs to user space

/*

*/
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

#[no_mangle]
pub extern "C" fn occlum_spawn(child_pid: *mut c_int, path: *const c_char,
    argv: *const *const c_char, envp: *const *const c_char) -> c_int
{
    let mut ret = 0;
    let path_str = unsafe {
        CStr::from_ptr(path as * const i8).to_string_lossy().into_owned()
    };
    match process::do_spawn(&path_str) {
        Ok(new_pid) => unsafe {
            *child_pid = new_pid as c_int;
            0
        },
        Err(e) => {
            e.errno.as_retval()
        }
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
