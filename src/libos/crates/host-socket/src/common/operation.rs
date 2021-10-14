use crate::prelude::*;

pub fn do_bind<A: Addr>(host_fd: HostFd, addr: &A) -> Result<()> {
    let fd = host_fd as i32;
    let (c_addr_storage, c_addr_len) = addr.to_c_storage();
    let c_addr_ptr = &c_addr_storage as *const _ as _;
    let c_addr_len = c_addr_len as u32;
    #[cfg(not(feature = "sgx"))]
    try_libc!(libc::bind(fd, c_addr_ptr, c_addr_len));
    #[cfg(feature = "sgx")]
    try_libc!(libc::ocall::bind(fd, c_addr_ptr, c_addr_len));
    Ok(())
}
