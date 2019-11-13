use super::*;

use fs::{AsUnixSocket, File, FileDesc, FileRef, UnixSocketFile};
use process::Process;
use util::mem_util::from_user;

pub fn do_sendmsg(fd: c_int, msg_ptr: *const msghdr, flags_c: c_int) -> Result<isize> {
    info!(
        "sendmsg: fd: {}, msg: {:?}, flags: 0x{:x}",
        fd, msg_ptr, flags_c
    );
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_ref = proc.get_files().lock().unwrap().get(fd as FileDesc)?;

    if let Ok(socket) = file_ref.as_socket() {
        let msg_c = {
            from_user::check_ptr(msg_ptr)?;
            let msg_c = unsafe { &*msg_ptr };
            msg_c.check_member_ptrs()?;
            msg_c
        };
        let msg = unsafe { MsgHdr::from_c(&msg_c)? };

        let flags = MsgFlags::from_u32(flags_c as u32)?;

        socket
            .sendmsg(&msg, flags)
            .map(|bytes_sent| bytes_sent as isize)
    } else if let Ok(socket) = file_ref.as_unix_socket() {
        return_errno!(EBADF, "does not support unix socket")
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

pub fn do_recvmsg(fd: c_int, msg_mut_ptr: *mut msghdr_mut, flags_c: c_int) -> Result<isize> {
    info!(
        "recvmsg: fd: {}, msg: {:?}, flags: 0x{:x}",
        fd, msg_mut_ptr, flags_c
    );
    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();
    let file_ref = proc.get_files().lock().unwrap().get(fd as FileDesc)?;

    if let Ok(socket) = file_ref.as_socket() {
        let msg_mut_c = {
            from_user::check_mut_ptr(msg_mut_ptr)?;
            let msg_mut_c = unsafe { &mut *msg_mut_ptr };
            msg_mut_c.check_member_ptrs()?;
            msg_mut_c
        };
        let mut msg_mut = unsafe { MsgHdrMut::from_c(msg_mut_c)? };

        let flags = MsgFlags::from_u32(flags_c as u32)?;

        socket
            .recvmsg(&mut msg_mut, flags)
            .map(|bytes_recvd| bytes_recvd as isize)
    } else if let Ok(socket) = file_ref.as_unix_socket() {
        return_errno!(EBADF, "does not support unix socket")
    } else {
        return_errno!(EBADF, "not a socket")
    }
}

#[allow(non_camel_case_types)]
trait c_msghdr_ext {
    fn check_member_ptrs(&self) -> Result<()>;
}

impl c_msghdr_ext for msghdr {
    // TODO: implement this!
    fn check_member_ptrs(&self) -> Result<()> {
        Ok(())
    }
    /*
            ///user space check
            pub unsafe fn check_from_user(user_hdr: *const msghdr) -> Result<()> {
                Self::check_pointer(user_hdr, from_user::check_ptr)
            }

            ///Check msghdr ptr
            pub unsafe fn check_pointer(
                user_hdr: *const msghdr,
                check_ptr: fn(*const u8) -> Result<()>,
            ) -> Result<()> {
                check_ptr(user_hdr as *const u8)?;

                if (*user_hdr).msg_name.is_null() ^ ((*user_hdr).msg_namelen == 0) {
                    return_errno!(EINVAL, "name length is invalid");
                }

                if (*user_hdr).msg_iov.is_null() ^ ((*user_hdr).msg_iovlen == 0) {
                    return_errno!(EINVAL, "iov length is invalid");
                }

                if (*user_hdr).msg_control.is_null() ^ ((*user_hdr).msg_controllen == 0) {
                    return_errno!(EINVAL, "control length is invalid");
                }

                if !(*user_hdr).msg_name.is_null() {
                    check_ptr((*user_hdr).msg_name as *const u8)?;
                }

                if !(*user_hdr).msg_iov.is_null() {
                    check_ptr((*user_hdr).msg_iov as *const u8)?;
                    let iov_slice = slice::from_raw_parts((*user_hdr).msg_iov, (*user_hdr).msg_iovlen);
                    for iov in iov_slice {
                        check_ptr(iov.iov_base as *const u8)?;
                    }
                }

                if !(*user_hdr).msg_control.is_null() {
                    check_ptr((*user_hdr).msg_control as *const u8)?;
                }
                Ok(())
            }
    */
}

impl c_msghdr_ext for msghdr_mut {
    fn check_member_ptrs(&self) -> Result<()> {
        Ok(())
    }
}
