use crate::{
    fs::{AccessMode, HostFd, IoEvents, IoNotifier, StatusFlags},
    prelude::{File, Result},
};

use super::socket_file::SocketFile;

impl File for SocketFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        self.read(buf)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        self.readv(bufs)
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        self.write(buf)
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        self.writev(bufs)
    }

    // fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
    //     if offset != 0 {
    //         return_errno!(ESPIPE, "a nonzero position is not supported");
    //     }
    //     self.read(buf)
    // }

    // fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
    //     if offset != 0 {
    //         return_errno!(ESPIPE, "a nonzero position is not supported");
    //     }
    //     self.write(buf)
    // }

    // fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
    //     let (bytes_recvd, _, _, _) = self.do_recvmsg(bufs, RecvFlags::empty(), None, None)?;
    //     Ok(bytes_recvd)
    // }

    // fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
    //     self.do_sendmsg(bufs, SendFlags::empty(), None, None)
    // }

    // fn seek(&self, pos: SeekFrom) -> Result<off_t> {
    //     return_errno!(ESPIPE, "Socket does not support seek")
    // }

    // fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<i32> {
    //     self.ioctl_impl(cmd)
    // }

    fn access_mode(&self) -> Result<AccessMode> {
        Ok(AccessMode::O_RDWR)
    }

    fn status_flags(&self) -> Result<StatusFlags> {
        Ok(self.status_flags())
        // let ret = try_libc!(libc::ocall::fcntl_arg0(
        //     self.raw_host_fd() as i32,
        //     libc::F_GETFL
        // ));
        // Ok(StatusFlags::from_bits_truncate(ret as u32))
    }

    fn set_status_flags(&self, new_status_flags: StatusFlags) -> Result<()> {
        self.set_status_flags(new_status_flags)
        // let raw_status_flags = (new_status_flags & STATUS_FLAGS_MASK).bits();
        // try_libc!(libc::ocall::fcntl_arg1(
        //     self.raw_host_fd() as i32,
        //     libc::F_SETFL,
        //     raw_status_flags as c_int
        // ));
        // Ok(())
    }

    fn poll_new(&self) -> IoEvents {
        let mask = IoEvents::all();
        self.poll(mask, None)
    }

    fn host_fd(&self) -> Option<&HostFd> {
        None
        // panic!()
        // Some(&self.host_fd_inner())
    }

    // fn notifier(&self) -> Option<&IoNotifier> {
    //     Some(&self.notifier())
    // }

    fn update_host_events(&self, ready: &IoEvents, mask: &IoEvents, trigger_notifier: bool) {
        panic!()
        // self.host_events.update(ready, mask, Ordering::Release);

        // if trigger_notifier {
        //     self.notifier.broadcast(ready);
        // }
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
