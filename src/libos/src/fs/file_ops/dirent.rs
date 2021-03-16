use super::*;

pub fn do_getdents64(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    getdents_common::<u8>(fd, buf)
}

pub fn do_getdents(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    getdents_common::<()>(fd, buf)
}

fn getdents_common<T: DirentType + Copy>(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
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
        // TODO: get ino and type from dirent
        let dirent = LinuxDirent::<T>::new(1, &name, DT_UNKNOWN);
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

const DT_UNKNOWN: u8 = 0;

#[repr(packed)] // Don't use 'C'. Or its size will align up to 8 bytes.
struct LinuxDirent<T: DirentType + Copy> {
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

impl<T: DirentType + Copy> LinuxDirent<T> {
    fn new(ino: u64, name: &str, d_type: u8) -> Self {
        let ori_len = if !T::at_the_end_of_linux_dirent() {
            core::mem::size_of::<LinuxDirent<T>>() + name.len() + 1
        } else {
            // pad the file type at the end
            core::mem::size_of::<LinuxDirent<T>>() + name.len() + 1 + core::mem::size_of::<u8>()
        };
        let len = align_up(ori_len, 8); // align up to 8 bytes
        Self {
            ino,
            offset: 0,
            reclen: len as u16,
            type_: T::set_type(d_type),
            name: [],
        }
    }

    fn len(&self) -> usize {
        self.reclen as usize
    }
}

impl<T: DirentType + Copy> Copy for LinuxDirent<T> {}

impl<T: DirentType + Copy> Clone for LinuxDirent<T> {
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

trait DirentType {
    fn set_type(d_type: u8) -> Self;
    fn at_the_end_of_linux_dirent() -> bool;
}

impl DirentType for u8 {
    fn set_type(d_type: u8) -> Self {
        d_type
    }
    fn at_the_end_of_linux_dirent() -> bool {
        false
    }
}
impl DirentType for () {
    fn set_type(d_type: u8) -> Self {
        Default::default()
    }
    fn at_the_end_of_linux_dirent() -> bool {
        true
    }
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

    fn try_write<T: DirentType + Copy>(
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
            if T::at_the_end_of_linux_dirent() {
                // pad zero bytes and file type at the end
                let mut ptr = name_ptr.add(name.len() + 1);
                let mut rest_len = {
                    let written_len = core::mem::size_of::<LinuxDirent<T>>() + name.len() + 1;
                    dirent.len() - written_len
                };
                while rest_len > 1 {
                    ptr.write(0);
                    ptr = ptr.add(1);
                    rest_len -= 1;
                }
                // the last one is file type
                ptr.write(DT_UNKNOWN);
            }
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
