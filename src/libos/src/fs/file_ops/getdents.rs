use super::*;
use core::marker::PhantomData;

pub async fn do_getdents64(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    getdents_common::<LinuxDirent64>(fd, buf).await
}

pub async fn do_getdents(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    getdents_common::<LinuxDirent>(fd, buf).await
}

async fn getdents_common<T: Dirent>(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    debug!(
        "getdents: fd: {}, buf: {:?}, buf_size: {}",
        fd,
        buf.as_ptr(),
        buf.len(),
    );

    let file_ref = current!().file(fd)?;
    let mut writer = DirentBufWriter::<T>::new(buf);
    let written_len = if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        async_file_handle.iterate_entries(&mut writer).await?
    } else {
        return_errno!(EBADF, "not an inode");
    };
    Ok(written_len)
}

struct DirentBufWriter<'a, T: Dirent> {
    buf: &'a mut [u8],
    written_len: usize,
    phantom: PhantomData<T>,
}

impl<'a, T: Dirent> DirentBufWriter<'a, T> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self {
            buf,
            written_len: 0,
            phantom: PhantomData,
        }
    }
}

impl<'a, T: Dirent> DirentWriter for DirentBufWriter<'a, T> {
    fn write_entry(&mut self, name: &str, ino: u64, type_: FileType) -> rcore_fs::vfs::Result<()> {
        let dirent: T = Dirent::new(name, ino, type_);
        if self.buf.len() - self.written_len < dirent.rec_len() {
            return Err(FsError::InvalidParam);
        }
        dirent
            .serialize(&mut self.buf[self.written_len..], name, type_)
            .map_err(|_| FsError::InvalidParam)?;
        self.written_len += dirent.rec_len();
        Ok(())
    }

    fn written_len(&self) -> usize {
        self.written_len
    }
}

trait Dirent: Sync + Send {
    fn new(name: &str, ino: u64, type_: FileType) -> Self;
    fn rec_len(&self) -> usize;
    fn serialize(&self, buf: &mut [u8], name: &str, type_: FileType) -> Result<()>;
}

/// Same with struct linux_dirent64
#[repr(packed)] // Don't use 'C'. Or its size will align up to 8 bytes.
#[derive(Debug, Clone, Copy)]
struct LinuxDirent64 {
    /// Inode number
    pub ino: u64,
    /// Offset to next structure
    pub offset: u64,
    /// Size of this dirent
    pub rec_len: u16,
    /// File type
    pub type_: DirentType,
    /// Filename (null-terminated)
    pub name: [u8; 0],
}

impl Dirent for LinuxDirent64 {
    fn new(name: &str, ino: u64, type_: FileType) -> Self {
        let ori_len = core::mem::size_of::<Self>() + name.len() + 1;
        let len = align_up(ori_len, 8); // align up to 8 bytes
        Self {
            ino,
            offset: 0,
            rec_len: len as u16,
            type_: DirentType::from_file_type(type_),
            name: [],
        }
    }

    fn rec_len(&self) -> usize {
        self.rec_len as usize
    }

    fn serialize(&self, buf: &mut [u8], name: &str, _type_: FileType) -> Result<()> {
        unsafe {
            let ptr = buf.as_mut_ptr() as *mut Self;
            ptr.write(*self);
            let name_ptr = ptr.add(1) as _;
            write_cstr(name_ptr, name);
        }
        Ok(())
    }
}

/// Same with struct linux_dirent
#[repr(packed)] // Don't use 'C'. Or its size will align up to 8 bytes.
#[derive(Debug, Clone, Copy)]
struct LinuxDirent {
    /// Inode number
    pub ino: u64,
    /// Offset to next structure
    pub offset: u64,
    /// Size of this dirent
    pub rec_len: u16,
    /// Filename (null-terminated)
    pub name: [u8; 0],
    /*
    /// Zero padding byte
    pub pad: [u8],
    /// File type
    pub type_: DirentType,
    */
}

impl Dirent for LinuxDirent {
    fn new(name: &str, ino: u64, type_: FileType) -> Self {
        let ori_len =
            core::mem::size_of::<Self>() + name.len() + 1 + core::mem::size_of::<FileType>();
        let len = align_up(ori_len, 8); // align up to 8 bytes
        Self {
            ino,
            offset: 0,
            rec_len: len as u16,
            name: [],
        }
    }

    fn rec_len(&self) -> usize {
        self.rec_len as usize
    }

    fn serialize(&self, buf: &mut [u8], name: &str, type_: FileType) -> Result<()> {
        unsafe {
            let ptr = buf.as_mut_ptr() as *mut Self;
            ptr.write(*self);
            let mut ptr = ptr.add(1) as *mut u8;
            write_cstr(ptr, name);
            // Pad zero bytes if necessary
            ptr = ptr.add(name.len() + 1);
            let mut remaining_len = {
                let written_len = core::mem::size_of::<Self>() + name.len() + 1;
                self.rec_len() - written_len
            };
            while remaining_len > 1 {
                ptr.write(0);
                ptr = ptr.add(1);
                remaining_len -= 1;
            }
            // Write the type at the end
            let dirent_type = DirentType::from_file_type(type_);
            ptr.write(dirent_type as u8);
        }
        Ok(())
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum DirentType {
    DT_UNKNOWN = 0,
    DT_FIFO = 1,
    DT_CHR = 2,
    DT_DIR = 4,
    DT_BLK = 6,
    DT_REG = 8,
    DT_LNK = 10,
    DT_SOCK = 12,
    DT_WHT = 14,
}

impl DirentType {
    fn from_file_type(file_type: FileType) -> DirentType {
        match file_type {
            FileType::File => DirentType::DT_REG,
            FileType::Dir => DirentType::DT_DIR,
            FileType::SymLink => DirentType::DT_LNK,
            FileType::CharDevice => DirentType::DT_CHR,
            FileType::BlockDevice => DirentType::DT_BLK,
            FileType::Socket => DirentType::DT_SOCK,
            FileType::NamedPipe => DirentType::DT_FIFO,
            _ => DirentType::DT_UNKNOWN,
        }
    }
}

/// Write a Rust string to C string
unsafe fn write_cstr(ptr: *mut u8, s: &str) {
    ptr.copy_from(s.as_ptr(), s.len());
    ptr.add(s.len()).write(0);
}
