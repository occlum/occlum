use super::*;

pub fn do_getdents64(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    getdents_common::<u8>(fd, buf)
}

pub fn do_getdents(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    getdents_common::<()>(fd, buf)
}

fn getdents_common<T: DirentType + Copy + Default>(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    debug!(
        "getdents: fd: {}, buf: {:?}, buf_size: {}",
        fd,
        buf.as_ptr(),
        buf.len(),
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
        let dirent = LinuxDirent::<T>::new(1, &name);
        if let Err(e) = writer.try_write(&dirent, &name) {
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

#[repr(packed)] // Don't use 'C'. Or its size will align up to 8 bytes.
struct LinuxDirent<T: DirentType + Copy + Default> {
    /// Inode number
    ino: u64,
    /// Offset to next structure
    offset: u64,
    /// Size of this dirent
    reclen: u16,
    /// File type
    type_: T,
    /// Filename (null-terminated)
    name: [u8; 0],
}

impl<T: DirentType + Copy + Default> LinuxDirent<T> {
    fn new(ino: u64, name: &str) -> Self {
        let ori_len = ::core::mem::size_of::<LinuxDirent<T>>() + name.len() + 1;
        let len = align_up(ori_len, 8); // align up to 8 bytes
        Self {
            ino,
            offset: 0,
            reclen: len as u16,
            type_: Default::default(),
            name: [],
        }
    }

    fn len(&self) -> usize {
        self.reclen as usize
    }
}

impl<T: DirentType + Copy + Default> Copy for LinuxDirent<T> {}

impl<T: DirentType + Copy + Default> Clone for LinuxDirent<T> {
    fn clone(&self) -> Self {
        Self {
            ino: self.ino,
            offset: self.offset,
            reclen: self.reclen,
            type_: self.type_,
            name: self.name,
        }
    }
}

trait DirentType {}

impl DirentType for u8 {}
impl DirentType for () {}

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

    fn try_write<T: DirentType + Copy + Default>(
        &mut self,
        dirent: &LinuxDirent<T>,
        name: &str,
    ) -> Result<()> {
        if self.rest_size < dirent.len() {
            return_errno!(EINVAL, "the given buffer is too small");
        }
        unsafe {
            let ptr = self.buf.as_mut_ptr().add(self.written_size) as *mut LinuxDirent<T>;
            ptr.write(*dirent);
            let name_ptr = ptr.add(1) as _;
            write_cstr(name_ptr, name);
        }
        self.rest_size -= dirent.len();
        self.written_size += dirent.len();
        Ok(())
    }
}

/// Write a Rust string to C string
unsafe fn write_cstr(ptr: *mut u8, s: &str) {
    ptr.copy_from(s.as_ptr(), s.len());
    ptr.add(s.len()).write(0);
}
