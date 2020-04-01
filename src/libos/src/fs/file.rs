use super::*;

macro_rules! return_op_unsupported_error {
    ($op_name: expr, $errno: expr) => {{
        let errno = $errno;
        // FIXME: use the safe core::any::type_name when we upgrade to Rust 1.38 or above
        let type_name = unsafe { core::intrinsics::type_name::<Self>() };
        let op_name = $op_name;
        let error = FileOpNotSupportedError::new(errno, type_name, op_name);
        return_errno!(error)
    }};
    ($op_name: expr) => {{
        return_op_unsupported_error!($op_name, ENOSYS)
    }};
}

pub trait File: Debug + Sync + Send + Any {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        return_op_unsupported_error!("read")
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        return_op_unsupported_error!("write")
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        return_op_unsupported_error!("read_at")
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        return_op_unsupported_error!("write_at")
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        return_op_unsupported_error!("readv")
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        return_op_unsupported_error!("writev")
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t> {
        return_op_unsupported_error!("seek")
    }

    fn metadata(&self) -> Result<Metadata> {
        return_op_unsupported_error!("metadata")
    }

    fn set_metadata(&self, metadata: &Metadata) -> Result<()> {
        return_op_unsupported_error!("set_metadata")
    }

    fn set_len(&self, len: u64) -> Result<()> {
        return_op_unsupported_error!("set_len")
    }

    fn read_entry(&self) -> Result<String> {
        return_op_unsupported_error!("read_entry", ENOTDIR)
    }

    fn sync_all(&self) -> Result<()> {
        Ok(())
    }

    fn sync_data(&self) -> Result<()> {
        Ok(())
    }

    fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<()> {
        return_op_unsupported_error!("ioctl")
    }

    fn get_access_mode(&self) -> Result<AccessMode> {
        return_op_unsupported_error!("get_access_mode")
    }

    fn get_status_flags(&self) -> Result<StatusFlags> {
        return_op_unsupported_error!("get_status_flags")
    }

    fn set_status_flags(&self, new_status_flags: StatusFlags) -> Result<()> {
        return_op_unsupported_error!("set_status_flags")
    }

    fn test_advisory_lock(&self, lock: &mut Flock) -> Result<()> {
        return_op_unsupported_error!("test_advisory_lock")
    }

    fn set_advisory_lock(&self, lock: &Flock) -> Result<()> {
        return_op_unsupported_error!("set_advisory_lock")
    }

    fn as_any(&self) -> &dyn Any;
}

pub type FileRef = Arc<Box<dyn File>>;

#[derive(Copy, Clone, Debug)]
struct FileOpNotSupportedError {
    errno: Errno,
    type_name: &'static str,
    op_name: &'static str,
}

impl FileOpNotSupportedError {
    pub fn new(
        errno: Errno,
        type_name: &'static str,
        op_name: &'static str,
    ) -> FileOpNotSupportedError {
        FileOpNotSupportedError {
            errno,
            type_name,
            op_name,
        }
    }
}

impl fmt::Display for FileOpNotSupportedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} does not support {}", self.type_name, self.op_name)
    }
}

impl ToErrno for FileOpNotSupportedError {
    fn errno(&self) -> Errno {
        self.errno
    }
}
