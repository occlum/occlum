use super::*;

use net::{AsSocket, SocketFile};
use process::Process;
use rcore_fs::vfs::{FileType, FsError, INode, Metadata, Timespec};
use std::io::{Read, Seek, SeekFrom, Write};
use std::sgxfs as fs_impl;
use {process, std};

pub use self::access::{do_access, do_faccessat, AccessFlags, AccessModes, AT_FDCWD};
use self::dev_null::DevNull;
use self::dev_random::DevRandom;
use self::dev_sgx::DevSgx;
use self::dev_zero::DevZero;
pub use self::fcntl::FcntlCmd;
pub use self::file::{File, FileRef, SgxFile, StdinFile, StdoutFile};
pub use self::file_flags::{AccessMode, CreationFlags, StatusFlags};
pub use self::file_table::{FileDesc, FileTable};
pub use self::inode_file::{AsINodeFile, INodeExt, INodeFile};
pub use self::io_multiplexing::*;
pub use self::ioctl::*;
pub use self::pipe::Pipe;
pub use self::root_inode::ROOT_INODE;
pub use self::unix_socket::{AsUnixSocket, UnixSocketFile};
use sgx_trts::libc::S_IWUSR;
use std::any::Any;
use std::mem::MaybeUninit;

mod access;
mod dev_null;
mod dev_random;
mod dev_sgx;
mod dev_zero;
mod fcntl;
mod file;
mod file_flags;
mod file_table;
mod hostfs;
mod inode_file;
mod io_multiplexing;
mod ioctl;
mod pipe;
mod root_inode;
mod sgx_impl;
mod unix_socket;

pub fn do_open(path: &str, flags: u32, mode: u32) -> Result<FileDesc> {
    info!(
        "open: path: {:?}, flags: {:#o}, mode: {:#o}",
        path, flags, mode
    );

    let current_ref = process::get_current();
    let mut proc = current_ref.lock().unwrap();

    let file = proc.open_file(path, flags, mode)?;
    let file_ref: Arc<Box<dyn File>> = Arc::new(file);

    let fd = {
        let creation_flags = CreationFlags::from_bits_truncate(flags);
        proc.get_files()
            .lock()
            .unwrap()
            .put(file_ref, creation_flags.must_close_on_spawn())
    };
    Ok(fd)
}

pub fn do_write(fd: FileDesc, buf: &[u8]) -> Result<usize> {
    info!("write: fd: {}", fd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.write(buf)
}

pub fn do_read(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    info!("read: fd: {}", fd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.read(buf)
}

pub fn do_writev(fd: FileDesc, bufs: &[&[u8]]) -> Result<usize> {
    info!("writev: fd: {}", fd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.writev(bufs)
}

pub fn do_readv(fd: FileDesc, bufs: &mut [&mut [u8]]) -> Result<usize> {
    info!("readv: fd: {}", fd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.readv(bufs)
}

pub fn do_pwrite(fd: FileDesc, buf: &[u8], offset: usize) -> Result<usize> {
    info!("pwrite: fd: {}, offset: {}", fd, offset);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.write_at(offset, buf)
}

pub fn do_pread(fd: FileDesc, buf: &mut [u8], offset: usize) -> Result<usize> {
    info!("pread: fd: {}, offset: {}", fd, offset);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.read_at(offset, buf)
}

pub fn do_stat(path: &str) -> Result<Stat> {
    warn!("stat is partial implemented as lstat");
    do_lstat(path)
}

pub fn do_fstat(fd: u32) -> Result<Stat> {
    info!("fstat: fd: {}", fd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    let stat = Stat::from(file_ref.metadata()?);
    // TODO: handle symlink
    Ok(stat)
}

pub fn do_lstat(path: &str) -> Result<Stat> {
    info!("lstat: path: {}", path);

    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let inode = current_process.lookup_inode(&path)?;
    let stat = Stat::from(inode.metadata()?);
    Ok(stat)
}

pub fn do_lseek(fd: FileDesc, offset: SeekFrom) -> Result<off_t> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.seek(offset)
}

pub fn do_fsync(fd: FileDesc) -> Result<()> {
    info!("fsync: fd: {}", fd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.sync_all()?;
    Ok(())
}

pub fn do_fdatasync(fd: FileDesc) -> Result<()> {
    info!("fdatasync: fd: {}", fd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.sync_data()?;
    Ok(())
}

pub fn do_truncate(path: &str, len: usize) -> Result<()> {
    info!("truncate: path: {:?}, len: {}", path, len);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    current_process.lookup_inode(&path)?.resize(len)?;
    Ok(())
}

pub fn do_ftruncate(fd: FileDesc, len: usize) -> Result<()> {
    info!("ftruncate: fd: {}, len: {}", fd, len);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.set_len(len as u64)?;
    Ok(())
}

pub fn do_getdents64(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    info!(
        "getdents64: fd: {}, buf: {:?}, buf_size: {}",
        fd,
        buf.as_ptr(),
        buf.len()
    );
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
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

pub fn do_close(fd: FileDesc) -> Result<()> {
    info!("close: fd: {}", fd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_table_ref = current_process.get_files();
    let mut file_table = file_table_ref.lock().unwrap();
    file_table.del(fd)?;
    Ok(())
}

pub fn do_ioctl(fd: FileDesc, cmd: &mut IoctlCmd) -> Result<()> {
    info!("ioctl: fd: {}, cmd: {:?}", fd, cmd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.ioctl(cmd)
}

pub fn do_pipe2(flags: u32) -> Result<[FileDesc; 2]> {
    info!("pipe2: flags: {:#x}", flags);
    let creation_flags = CreationFlags::from_bits_truncate(flags);
    let current_ref = process::get_current();
    let current = current_ref.lock().unwrap();
    let pipe = Pipe::new()?;

    let file_table_ref = current.get_files();
    let mut file_table = file_table_ref.lock().unwrap();
    let close_on_spawn = creation_flags.must_close_on_spawn();
    let reader_fd = file_table.put(Arc::new(Box::new(pipe.reader)), close_on_spawn);
    let writer_fd = file_table.put(Arc::new(Box::new(pipe.writer)), close_on_spawn);
    info!("pipe2: reader_fd: {}, writer_fd: {}", reader_fd, writer_fd);
    Ok([reader_fd, writer_fd])
}

pub fn do_dup(old_fd: FileDesc) -> Result<FileDesc> {
    let current_ref = process::get_current();
    let current = current_ref.lock().unwrap();
    let file_table_ref = current.get_files();
    let mut file_table = file_table_ref.lock().unwrap();
    let file = file_table.get(old_fd)?;
    let new_fd = file_table.put(file, false);
    Ok(new_fd)
}

pub fn do_dup2(old_fd: FileDesc, new_fd: FileDesc) -> Result<FileDesc> {
    let current_ref = process::get_current();
    let current = current_ref.lock().unwrap();
    let file_table_ref = current.get_files();
    let mut file_table = file_table_ref.lock().unwrap();
    let file = file_table.get(old_fd)?;
    if old_fd != new_fd {
        file_table.put_at(new_fd, file, false);
    }
    Ok(new_fd)
}

pub fn do_dup3(old_fd: FileDesc, new_fd: FileDesc, flags: u32) -> Result<FileDesc> {
    let creation_flags = CreationFlags::from_bits_truncate(flags);
    let current_ref = process::get_current();
    let current = current_ref.lock().unwrap();
    let file_table_ref = current.get_files();
    let mut file_table = file_table_ref.lock().unwrap();
    let file = file_table.get(old_fd)?;
    if old_fd == new_fd {
        return_errno!(EINVAL, "old_fd must not be equal to new_fd");
    }
    file_table.put_at(new_fd, file, creation_flags.must_close_on_spawn());
    Ok(new_fd)
}

pub fn do_sync() -> Result<()> {
    info!("sync:");
    ROOT_INODE.fs().sync()?;
    Ok(())
}

pub fn do_chdir(path: &str) -> Result<()> {
    let current_ref = process::get_current();
    let mut current_process = current_ref.lock().unwrap();
    info!("chdir: path: {:?}", path);

    let inode = current_process.lookup_inode(path)?;
    let info = inode.metadata()?;
    if info.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "");
    }
    current_process.change_cwd(path);
    Ok(())
}

pub fn do_rename(oldpath: &str, newpath: &str) -> Result<()> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    info!("rename: oldpath: {:?}, newpath: {:?}", oldpath, newpath);

    let (old_dir_path, old_file_name) = split_path(&oldpath);
    let (new_dir_path, new_file_name) = split_path(&newpath);
    let old_dir_inode = current_process.lookup_inode(old_dir_path)?;
    let new_dir_inode = current_process.lookup_inode(new_dir_path)?;
    // TODO: support to modify file's absolute path
    old_dir_inode.move_(old_file_name, &new_dir_inode, new_file_name)?;
    Ok(())
}

pub fn do_mkdir(path: &str, mode: usize) -> Result<()> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    // TODO: check pathname
    info!("mkdir: path: {:?}, mode: {:#o}", path, mode);

    let (dir_path, file_name) = split_path(&path);
    let inode = current_process.lookup_inode(dir_path)?;
    if inode.find(file_name).is_ok() {
        return_errno!(EEXIST, "");
    }
    if !inode.allow_write()? {
        return_errno!(EPERM, "dir cannot be written");
    }
    inode.create(file_name, FileType::Dir, mode as u32)?;
    Ok(())
}

pub fn do_rmdir(path: &str) -> Result<()> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    info!("rmdir: path: {:?}", path);

    let (dir_path, file_name) = split_path(&path);
    let dir_inode = current_process.lookup_inode(dir_path)?;
    let file_inode = dir_inode.find(file_name)?;
    if file_inode.metadata()?.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "rmdir on not directory");
    }
    dir_inode.unlink(file_name)?;
    Ok(())
}

pub fn do_link(oldpath: &str, newpath: &str) -> Result<()> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    info!("link: oldpath: {:?}, newpath: {:?}", oldpath, newpath);

    let (new_dir_path, new_file_name) = split_path(&newpath);
    let inode = current_process.lookup_inode(&oldpath)?;
    let new_dir_inode = current_process.lookup_inode(new_dir_path)?;
    new_dir_inode.link(new_file_name, &inode)?;
    Ok(())
}

pub fn do_unlink(path: &str) -> Result<()> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    info!("unlink: path: {:?}", path);

    let (dir_path, file_name) = split_path(&path);
    let dir_inode = current_process.lookup_inode(dir_path)?;
    let file_inode = dir_inode.find(file_name)?;
    if file_inode.metadata()?.type_ == FileType::Dir {
        return_errno!(EISDIR, "unlink on directory");
    }
    dir_inode.unlink(file_name)?;
    Ok(())
}

pub fn do_sendfile(
    out_fd: FileDesc,
    in_fd: FileDesc,
    offset: Option<off_t>,
    count: usize,
) -> Result<(usize, usize)> {
    // (len, offset)
    info!(
        "sendfile: out: {}, in: {}, offset: {:?}, count: {}",
        out_fd, in_fd, offset, count
    );
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_table_ref = current_process.get_files();
    let mut file_table = file_table_ref.lock().unwrap();

    let in_file = file_table.get(in_fd)?;
    let out_file = file_table.get(out_fd)?;
    let mut buffer: [u8; 1024 * 11] = unsafe { MaybeUninit::uninit().assume_init() };

    let mut read_offset = match offset {
        Some(offset) => offset,
        None => in_file.seek(SeekFrom::Current(0))?,
    } as usize;

    // read from specified offset and write new offset back
    let mut bytes_read = 0;
    while bytes_read < count {
        let len = min(buffer.len(), count - bytes_read);
        let read_len = in_file.read_at(read_offset, &mut buffer[..len])?;
        if read_len == 0 {
            break;
        }
        bytes_read += read_len;
        read_offset += read_len;
        let mut bytes_written = 0;
        while bytes_written < read_len {
            let write_len = out_file.write(&buffer[bytes_written..])?;
            if write_len == 0 {
                return_errno!(EBADF, "sendfile write return 0");
            }
            bytes_written += write_len;
        }
    }

    if offset.is_none() {
        in_file.seek(SeekFrom::Current(bytes_read as i64))?;
    }
    Ok((bytes_read, read_offset))
}

extern "C" {
    fn occlum_ocall_sync() -> sgx_status_t;
}

impl Process {
    /// Open a file on the process. But DO NOT add it to file table.
    pub fn open_file(&self, path: &str, flags: u32, mode: u32) -> Result<Box<dyn File>> {
        if path == "/dev/null" {
            return Ok(Box::new(DevNull));
        }
        if path == "/dev/zero" {
            return Ok(Box::new(DevZero));
        }
        if path == "/dev/random" || path == "/dev/urandom" || path == "/dev/arandom" {
            return Ok(Box::new(DevRandom));
        }
        if path == "/dev/sgx" {
            return Ok(Box::new(DevSgx));
        }
        let creation_flags = CreationFlags::from_bits_truncate(flags);
        let inode = if creation_flags.can_create() {
            let (dir_path, file_name) = split_path(&path);
            let dir_inode = self.lookup_inode(dir_path)?;
            match dir_inode.find(file_name) {
                Ok(file_inode) => {
                    if creation_flags.is_exclusive() {
                        return_errno!(EEXIST, "file exists");
                    }
                    file_inode
                }
                Err(FsError::EntryNotFound) => {
                    if !dir_inode.allow_write()? {
                        return_errno!(EPERM, "file cannot be created");
                    }
                    dir_inode.create(file_name, FileType::File, mode)?
                }
                Err(e) => return Err(Error::from(e)),
            }
        } else {
            self.lookup_inode(&path)?
        };
        let abs_path = self.convert_to_abs_path(&path);
        Ok(Box::new(INodeFile::open(inode, &abs_path, flags)?))
    }

    /// Lookup INode from the cwd of the process
    pub fn lookup_inode(&self, path: &str) -> Result<Arc<dyn INode>> {
        debug!("lookup_inode: cwd: {:?}, path: {:?}", self.get_cwd(), path);
        if path.len() > 0 && path.as_bytes()[0] == b'/' {
            // absolute path
            let abs_path = path.trim_start_matches('/');
            let inode = ROOT_INODE.lookup(abs_path)?;
            Ok(inode)
        } else {
            // relative path
            let cwd = self.get_cwd().trim_start_matches('/');
            let inode = ROOT_INODE.lookup(cwd)?.lookup(path)?;
            Ok(inode)
        }
    }

    /// Convert the path to be absolute
    pub fn convert_to_abs_path(&self, path: &str) -> String {
        debug!(
            "convert_to_abs_path: cwd: {:?}, path: {:?}",
            self.get_cwd(),
            path
        );
        if path.len() > 0 && path.as_bytes()[0] == b'/' {
            // path is absolute path already
            return path.to_owned();
        }
        let cwd = {
            if !self.get_cwd().ends_with("/") {
                self.get_cwd().to_owned() + "/"
            } else {
                self.get_cwd().to_owned()
            }
        };
        cwd + path
    }
}

/// Split a `path` str to `(base_path, file_name)`
fn split_path(path: &str) -> (&str, &str) {
    let mut split = path.trim_end_matches('/').rsplitn(2, '/');
    let file_name = split.next().unwrap();
    let mut dir_path = split.next().unwrap_or(".");
    if dir_path == "" {
        dir_path = "/";
    }
    (dir_path, file_name)
}

#[repr(packed)] // Don't use 'C'. Or its size will align up to 8 bytes.
pub struct LinuxDirent64 {
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

#[repr(C)]
pub struct Stat {
    /// ID of device containing file
    dev: u64,
    /// inode number
    ino: u64,
    /// number of hard links
    nlink: u64,

    /// file type and mode
    mode: StatMode,
    /// user ID of owner
    uid: u32,
    /// group ID of owner
    gid: u32,
    /// padding
    _pad0: u32,
    /// device ID (if special file)
    rdev: u64,
    /// total size, in bytes
    size: u64,
    /// blocksize for filesystem I/O
    blksize: u64,
    /// number of 512B blocks allocated
    blocks: u64,

    /// last access time
    atime: Timespec,
    /// last modification time
    mtime: Timespec,
    /// last status change time
    ctime: Timespec,
}

bitflags! {
    pub struct StatMode: u32 {
        const NULL  = 0;
        /// Type
        const TYPE_MASK = 0o170000;
        /// FIFO
        const FIFO  = 0o010000;
        /// character device
        const CHAR  = 0o020000;
        /// directory
        const DIR   = 0o040000;
        /// block device
        const BLOCK = 0o060000;
        /// ordinary regular file
        const FILE  = 0o100000;
        /// symbolic link
        const LINK  = 0o120000;
        /// socket
        const SOCKET = 0o140000;

        /// Set-user-ID on execution.
        const SET_UID = 0o4000;
        /// Set-group-ID on execution.
        const SET_GID = 0o2000;

        /// Read, write, execute/search by owner.
        const OWNER_MASK = 0o700;
        /// Read permission, owner.
        const OWNER_READ = 0o400;
        /// Write permission, owner.
        const OWNER_WRITE = 0o200;
        /// Execute/search permission, owner.
        const OWNER_EXEC = 0o100;

        /// Read, write, execute/search by group.
        const GROUP_MASK = 0o70;
        /// Read permission, group.
        const GROUP_READ = 0o40;
        /// Write permission, group.
        const GROUP_WRITE = 0o20;
        /// Execute/search permission, group.
        const GROUP_EXEC = 0o10;

        /// Read, write, execute/search by others.
        const OTHER_MASK = 0o7;
        /// Read permission, others.
        const OTHER_READ = 0o4;
        /// Write permission, others.
        const OTHER_WRITE = 0o2;
        /// Execute/search permission, others.
        const OTHER_EXEC = 0o1;
    }
}

impl StatMode {
    fn from_type_mode(type_: FileType, mode: u16) -> Self {
        let type_ = match type_ {
            FileType::File => StatMode::FILE,
            FileType::Dir => StatMode::DIR,
            FileType::SymLink => StatMode::LINK,
            FileType::CharDevice => StatMode::CHAR,
            FileType::BlockDevice => StatMode::BLOCK,
            FileType::Socket => StatMode::SOCKET,
            FileType::NamedPipe => StatMode::FIFO,
            _ => StatMode::NULL,
        };
        let mode = StatMode::from_bits_truncate(mode as u32);
        type_ | mode
    }
}

impl From<Metadata> for Stat {
    fn from(info: Metadata) -> Self {
        Stat {
            dev: info.dev as u64,
            ino: info.inode as u64,
            mode: StatMode::from_type_mode(info.type_, info.mode as u16),
            nlink: info.nlinks as u64,
            uid: info.uid as u32,
            gid: info.gid as u32,
            rdev: 0,
            size: info.size as u64,
            blksize: info.blk_size as u64,
            blocks: info.blocks as u64,
            atime: info.atime,
            mtime: info.mtime,
            ctime: info.ctime,
            _pad0: 0,
        }
    }
}

/// Write a Rust string to C string
pub unsafe fn write_cstr(ptr: *mut u8, s: &str) {
    ptr.copy_from(s.as_ptr(), s.len());
    ptr.add(s.len()).write(0);
}

pub fn do_fcntl(fd: FileDesc, cmd: &FcntlCmd) -> Result<isize> {
    info!("fcntl: fd: {:?}, cmd: {:?}", &fd, cmd);
    let current_ref = process::get_current();
    let mut current = current_ref.lock().unwrap();
    let file_table_ref = current.get_files();
    let ret = fcntl::do_fcntl(file_table_ref, fd, cmd)?;
    Ok(ret)
}

pub fn do_readlink(path: &str, buf: &mut [u8]) -> Result<usize> {
    info!("readlink: path: {:?}", path);
    let file_path = {
        if path == "/proc/self/exe" {
            let current_ref = process::get_current();
            let current = current_ref.lock().unwrap();
            current.get_elf_path().to_owned()
        } else if path.starts_with("/proc/self/fd") {
            let fd = path
                .trim_start_matches("/proc/self/fd/")
                .parse::<FileDesc>()
                .map_err(|e| errno!(EBADF, "Invalid file descriptor"))?;
            let current_ref = process::get_current();
            let current = current_ref.lock().unwrap();
            let file_ref = current.get_files().lock().unwrap().get(fd)?;
            if let Ok(inode_file) = file_ref.as_inode_file() {
                inode_file.get_abs_path().to_owned()
            } else {
                // TODO: support special device files
                return_errno!(EINVAL, "not a normal file link")
            }
        } else {
            // TODO: support symbolic links
            return_errno!(EINVAL, "not a symbolic link")
        }
    };
    let len = file_path.len().min(buf.len());
    buf[0..len].copy_from_slice(&file_path.as_bytes()[0..len]);
    Ok(len)
}
