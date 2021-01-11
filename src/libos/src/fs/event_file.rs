use super::*;

use atomic::{Atomic, Ordering};

/// Native Linux eventfd
// TODO: move the implementaion of eventfd into libos to defend against Iago attacks from OCalls
#[derive(Debug)]
pub struct EventFile {
    host_fd: HostFd,
    host_events: Atomic<IoEvents>,
    notifier: IoNotifier,
}

impl EventFile {
    pub fn new(init_val: u32, flags: EventCreationFlags) -> Result<Self> {
        let raw_host_fd = try_libc!({
            let mut ret: i32 = 0;
            let status = occlum_ocall_eventfd(&mut ret, init_val, flags.bits());
            assert!(status == sgx_status_t::SGX_SUCCESS);
            ret
        }) as FileDesc;
        let host_fd = HostFd::new(raw_host_fd);
        let host_events = Atomic::new(IoEvents::empty());
        let notifier = IoNotifier::new();
        Ok(Self {
            host_fd,
            host_events,
            notifier,
        })
    }

    pub fn host_fd(&self) -> c_int {
        self.host_fd.to_raw() as c_int
    }
}

bitflags! {
    pub struct EventCreationFlags: i32 {
        /// Provides semaphore-like semantics for reads from the new file descriptor
        const EFD_SEMAPHORE = 1 << 0;
        /// Non-blocking
        const EFD_NONBLOCK  = 1 << 11;
        /// Close on exec
        const EFD_CLOEXEC   = 1 << 19;
    }
}

extern "C" {
    fn occlum_ocall_eventfd(ret: *mut i32, init_val: u32, flags: i32) -> sgx_status_t;
}

impl File for EventFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let (buf_ptr, buf_len) = buf.as_mut().as_mut_ptr_and_len();

        let ret = try_libc!(libc::ocall::read(
            self.host_fd.to_raw() as i32,
            buf_ptr as *mut c_void,
            buf_len
        )) as usize;
        assert!(ret <= buf.len());
        Ok(ret)
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        let (buf_ptr, buf_len) = buf.as_ptr_and_len();
        let ret = try_libc!(libc::ocall::write(
            self.host_fd.to_raw() as i32,
            buf_ptr as *const c_void,
            buf_len
        )) as usize;
        assert!(ret <= buf.len());
        Ok(ret)
    }

    fn access_mode(&self) -> Result<AccessMode> {
        Ok(AccessMode::O_RDWR)
    }

    fn status_flags(&self) -> Result<StatusFlags> {
        let ret = try_libc!(libc::ocall::fcntl_arg0(self.host_fd(), libc::F_GETFL));
        Ok(StatusFlags::from_bits_truncate(ret as u32))
    }

    fn set_status_flags(&self, new_status_flags: StatusFlags) -> Result<()> {
        let valid_flags_mask = StatusFlags::O_APPEND
            | StatusFlags::O_ASYNC
            | StatusFlags::O_DIRECT
            | StatusFlags::O_NOATIME
            | StatusFlags::O_NONBLOCK;
        let raw_status_flags = (new_status_flags & valid_flags_mask).bits();
        try_libc!(libc::ocall::fcntl_arg1(
            self.host_fd(),
            libc::F_SETFL,
            raw_status_flags as c_int
        ));
        Ok(())
    }

    fn poll_new(&self) -> IoEvents {
        self.host_events.load(Ordering::Acquire)
    }

    fn notifier(&self) -> Option<&IoNotifier> {
        Some(&self.notifier)
    }

    fn host_fd(&self) -> Option<&HostFd> {
        Some(&self.host_fd)
    }

    fn update_host_events(&self, ready: &IoEvents, mask: &IoEvents, trigger_notifier: bool) {
        self.host_events.update(ready, mask, Ordering::Release);

        if trigger_notifier {
            self.notifier.broadcast(ready);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub trait AsEvent {
    fn as_event(&self) -> Result<&EventFile>;
}

impl AsEvent for FileRef {
    fn as_event(&self) -> Result<&EventFile> {
        self.as_any()
            .downcast_ref::<EventFile>()
            .ok_or_else(|| errno!(EBADF, "not an event file"))
    }
}
