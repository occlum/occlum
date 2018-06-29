use sgx_types::*;

// Use the internal syscall wrappers from sgx_tstd
use std::libc_fs as fs;
use std::libc_io as io;


#[no_mangle]
pub unsafe extern "C" fn sys_open(path: * const c_char, flags: c_int, mode: c_int) -> c_int {
    fs::open64(path, flags, mode)
}

#[no_mangle]
pub unsafe extern "C" fn sys_close(fd: c_int) -> c_int {
    io::close(fd)
}

#[no_mangle]
pub unsafe extern "C" fn sys_read(fd: c_int, buf: * mut c_void, size: size_t) -> ssize_t {
    io::read(fd, buf, size)
}

#[no_mangle]
pub unsafe extern "C" fn sys_write(fd: c_int, buf: * const c_void, size: size_t) -> ssize_t {
    io::write(fd, buf, size)
}
