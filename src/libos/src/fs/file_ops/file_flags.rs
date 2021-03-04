use super::*;

#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
#[repr(u8)]
pub enum AccessMode {
    /// read only
    O_RDONLY = 0,
    /// write only
    O_WRONLY = 1,
    /// read write
    O_RDWR = 2,
}

impl AccessMode {
    pub fn readable(&self) -> bool {
        match *self {
            AccessMode::O_RDONLY | AccessMode::O_RDWR => true,
            _ => false,
        }
    }

    pub fn writable(&self) -> bool {
        match *self {
            AccessMode::O_WRONLY | AccessMode::O_RDWR => true,
            _ => false,
        }
    }
}

impl AccessMode {
    pub fn from_u32(flags: u32) -> Result<Self> {
        let bits = flags & 0b11;
        if bits > AccessMode::O_RDWR as u32 {
            return_errno!(EINVAL, "invalid bits for access mode")
        }
        Ok(unsafe { core::mem::transmute(bits as u8) })
    }
}

bitflags! {
    pub struct CreationFlags: u32 {
        /// create file if it does not exist
        const O_CREAT = 1 << 6;
        /// error if CREATE and the file exists
        const O_EXCL = 1 << 7;
        /// not become the process's controlling terminal
        const O_NOCTTY = 1 << 8;
        /// truncate file upon open
        const O_TRUNC = 1 << 9;
        /// file is a directory
        const O_DIRECTORY = 1 << 16;
        /// pathname is not a symbolic link
        const O_NOFOLLOW = 1 << 17;
        /// close on exec
        const O_CLOEXEC = 1 << 19;
        /// create an unnamed temporary regular file
        /// O_TMPFILE is (_O_TMPFILE | O_DIRECTORY)
        const _O_TMPFILE = 1 << 22;
    }
}

impl CreationFlags {
    pub fn must_close_on_spawn(&self) -> bool {
        self.contains(CreationFlags::O_CLOEXEC)
    }

    pub fn can_create(&self) -> bool {
        self.contains(CreationFlags::O_CREAT)
    }

    pub fn is_exclusive(&self) -> bool {
        self.contains(CreationFlags::O_EXCL)
    }

    pub fn no_follow_symlink(&self) -> bool {
        self.contains(CreationFlags::O_NOFOLLOW)
    }

    pub fn must_be_directory(&self) -> bool {
        if self.contains(CreationFlags::_O_TMPFILE) {
            warn!("O_TMPFILE is not supported, handle it as O_DIRECTORY");
            return true;
        }
        self.contains(CreationFlags::O_DIRECTORY)
    }

    pub fn should_truncate(&self) -> bool {
        self.contains(CreationFlags::O_TRUNC)
    }
}

bitflags! {
    pub struct StatusFlags: u32 {
        /// append on each write
        const O_APPEND = 1 << 10;
        /// non block
        const O_NONBLOCK = 1 << 11;
        /// synchronized I/O, data
        const O_DSYNC = 1 << 12;
        /// signal-driven I/O
        const O_ASYNC = 1 << 13;
        /// direct I/O
        const O_DIRECT = 1 << 14;
        /// on x86_64, O_LARGEFILE is 0
        /// not update st_atime
        const O_NOATIME = 1 << 18;
        /// synchronized I/O, data and metadata
        const _O_SYNC = 1 << 20;
        /// equivalent of POSIX.1's O_EXEC
        const O_PATH = 1 << 21;
    }
}

impl StatusFlags {
    pub fn always_append(&self) -> bool {
        self.contains(StatusFlags::O_APPEND)
    }

    pub fn is_fast_open(&self) -> bool {
        self.contains(StatusFlags::O_PATH)
    }
}
