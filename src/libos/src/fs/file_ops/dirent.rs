use super::*;

#[repr(packed)] // Don't use 'C'. Or its size will align up to 8 bytes.
struct LinuxDirent64 {
    /// Inode number
    ino: u64,
    /// Offset to next structure
    offset: u64,
    /// Size of this dirent
    reclen: u16,
    /// File type
    type_: u8,
    /// Filename (null-terminated)
    name: [u8; 0],
}

struct DirentBufWriter<'a> {
    buf: &'a mut [u8],
    rest_size: usize,
    written_size: usize,
}

impl<'a> DirentBufWriter<'a> {
    unsafe fn new(buf: &'a mut [u8]) -> Self {
        let rest_size = buf.len();
        DirentBufWriter {
            buf,
            rest_size,
            written_size: 0,
        }
    }
    fn try_write(&mut self, inode: u64, type_: u8, name: &str) -> Result<()> {
        let len = ::core::mem::size_of::<LinuxDirent64>() + name.len() + 1;
        let len = (len + 7) / 8 * 8; // align up
        if self.rest_size < len {
            return_errno!(EINVAL, "the given buffer is too small");
        }
        let dent = LinuxDirent64 {
            ino: inode,
            offset: 0,
            reclen: len as u16,
            type_,
            name: [],
        };
        unsafe {
            let ptr = self.buf.as_mut_ptr().add(self.written_size) as *mut LinuxDirent64;
            ptr.write(dent);
            let name_ptr = ptr.add(1) as _;
            write_cstr(name_ptr, name);
        }
        self.rest_size -= len;
        self.written_size += len;
        Ok(())
    }
}

/// Write a Rust string to C string
unsafe fn write_cstr(ptr: *mut u8, s: &str) {
    ptr.copy_from(s.as_ptr(), s.len());
    ptr.add(s.len()).write(0);
}

pub fn do_getdents64(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    debug!(
        "getdents64: fd: {}, buf: {:?}, buf_size: {}",
        fd,
        buf.as_ptr(),
        buf.len()
    );
    let file_ref = current!().file(fd)?;
    let info = file_ref.metadata()?;
    if info.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "");
    }
    let mut writer = unsafe { DirentBufWriter::new(buf) };
    loop {
        let name = match file_ref.read_entry() {
            Err(e) => {
                let errno = e.errno();
                if errno == ENOENT {
                    break;
                }
                return Err(e.cause_err(|_| errno!(errno, "failed to read entry")));
            }
            Ok(name) => name,
        };
        // TODO: get ino from dirent
        if let Err(e) = writer.try_write(0, 0, &name) {
            file_ref.seek(SeekFrom::Current(-1))?;
            if writer.written_size == 0 {
                return Err(e);
            } else {
                break;
            }
        }
    }
    Ok(writer.written_size)
}
