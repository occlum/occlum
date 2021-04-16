use std::convert::TryInto;

use async_io::file::{AccessMode, PollableFile, StatusFlags};
use async_io::poll::{Events, Pollee, Poller};
use async_io::prelude::*;
use atomic::{Atomic, Ordering};

use crate::prelude::*;

pub fn do_eventfd(init_val: u32, flags: EventFileFlags) -> Result<FileDesc> {
    debug!("do_eventfd: init_val: {:?}, flags: {:?}", &init_val, &flags);

    let is_semaphore = flags.contains(EventFileFlags::EFD_SEMAPHORE);
    let close_on_spawn = flags.contains(EventFileFlags::EFD_CLOEXEC);
    let status_flags = if flags.contains(EventFileFlags::EFD_NONBLOCK) {
        StatusFlags::O_NONBLOCK
    } else {
        StatusFlags::empty()
    };

    let event_file = EventFile::new(init_val as u64, is_semaphore, status_flags)?;
    let file_ref = FileRef::from_pollable(Arc::new(event_file));
    let event_fd = current!().add_file(file_ref, close_on_spawn);
    Ok(event_fd)
}

bitflags! {
    pub struct EventFileFlags: i32 {
        /// Provides semaphore-like semantics for reads from the new file descriptor
        const EFD_SEMAPHORE = 1 << 0;
        /// Non-blocking
        const EFD_NONBLOCK  = 1 << 11;
        /// Close on exec
        const EFD_CLOEXEC   = 1 << 19;
    }
}

pub struct EventFile {
    // Invariance: 0 <= val <= u64::max_value() - 1
    val: SgxMutex<u64>,
    pollee: Pollee,
    is_semaphore: bool,
    flags: Atomic<StatusFlags>,
}

impl EventFile {
    pub fn new(init_val: u64, is_semaphore: bool, flags: StatusFlags) -> Result<Self> {
        if init_val > u64::max_value() - 1 {
            return_errno!(EINVAL, "value is too big");
        }
        let init_events = if init_val > 0 {
            Events::IN | Events::OUT
        } else {
            Events::OUT
        };
        check_status_flags(flags)?;
        Ok(Self {
            val: SgxMutex::new(init_val as u64),
            pollee: Pollee::new(init_events),
            is_semaphore,
            flags: Atomic::new(flags),
        })
    }
}

impl PollableFile for EventFile {
    fn write(&self, buf: &[u8]) -> Result<usize> {
        let new_val = slice_to_u64(buf)?;
        if new_val == u64::max_value() {
            return_errno!(EINVAL, "the value is too big");
        }
        if new_val == 0 {
            return Ok(8);
        }

        let mut val = self.val.lock().unwrap();
        let max_new_val = u64::max_value() - 1 - *val;
        if new_val > max_new_val {
            return_errno!(EAGAIN, "try again after read");
        }
        debug_assert!(*val + new_val <= u64::max_value() - 1);

        *val += new_val;
        self.pollee.add_events(Events::IN);

        Ok(8)
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        if buf.len() < 8 {
            return_errno!(EINVAL, "buffer is too small");
        }
        let buf = &mut buf[0..8];

        let mut val = self.val.lock().unwrap();
        if *val == 0 {
            return_errno!(EAGAIN, "try again after write");
        }

        let read_val = if self.is_semaphore { 1 } else { *val };
        *val -= read_val;

        self.pollee.add_events(Events::OUT);
        if *val == 0 {
            self.pollee.del_events(Events::IN);
        }

        let bytes = read_val.to_ne_bytes();
        buf.copy_from_slice(&bytes);
        Ok(8)
    }

    fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        self.pollee.poll_by(mask, poller)
    }

    fn access_mode(&self) -> Result<AccessMode> {
        Ok(AccessMode::O_RDWR)
    }

    fn status_flags(&self) -> Result<StatusFlags> {
        Ok(self.flags.load(Ordering::Relaxed))
    }

    fn set_status_flags(&self, new_status: StatusFlags) -> Result<()> {
        check_status_flags(new_status)?;
        self.flags.store(new_status, Ordering::Relaxed);
        Ok(())
    }
}

impl std::fmt::Debug for EventFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventFile")
            .field("val", &*self.val.lock().unwrap())
            .field("is_semaphore", &self.is_semaphore)
            .finish()
    }
}

fn check_status_flags(flags: StatusFlags) -> Result<()> {
    let VALID_FLAGS: StatusFlags = StatusFlags::O_NONBLOCK;
    if !VALID_FLAGS.contains(flags) {
        return_errno!(EINVAL, "invalid flags");
    }
    Ok(())
}

fn slice_to_u64(buf: &[u8]) -> Result<u64> {
    if buf.len() < 8 {
        return_errno!(EINVAL, "buffer is too small");
    }
    let bytes: [u8; 8] = (&buf[0..8]).try_into().unwrap();
    let val = u64::from_ne_bytes(bytes);
    Ok(val)
}
