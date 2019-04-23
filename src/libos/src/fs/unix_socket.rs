use super::*;
use std::collections::btree_map::BTreeMap;
use util::ring_buf::{RingBufReader, RingBufWriter, RingBuf};
use std::sync::SgxMutex as Mutex;
use alloc::prelude::ToString;
use std::fmt;

pub struct UnixSocketFile {
    inner: Mutex<UnixSocket>
}

impl File for UnixSocketFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.read(buf)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.write(buf)
    }

    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> Result<usize, Error> {
        self.read(buf)
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize, Error> {
        self.write(buf)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize, Error> {
        let mut inner = self.inner.lock().unwrap();
        let mut total_len = 0;
        for buf in bufs {
            match inner.read(buf) {
                Ok(len) => {
                    total_len += len;
                }
                Err(_) if total_len != 0 => break,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(total_len)
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize, Error> {
        let mut inner = self.inner.lock().unwrap();
        let mut total_len = 0;
        for buf in bufs {
            match inner.write(buf) {
                Ok(len) => {
                    total_len += len;
                }
                Err(_) if total_len != 0 => break,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(total_len)
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t, Error> {
        errno!(ESPIPE, "UnixSocket does not support seek")
    }

    fn metadata(&self) -> Result<Metadata, Error> {
        Ok(Metadata {
            dev: 0,
            inode: 0,
            size: 0,
            blk_size: 0,
            blocks: 0,
            atime: Timespec { sec: 0, nsec: 0 },
            mtime: Timespec { sec: 0, nsec: 0 },
            ctime: Timespec { sec: 0, nsec: 0 },
            type_: FileType::Socket,
            mode: 0,
            nlinks: 0,
            uid: 0,
            gid: 0
        })
    }

    fn set_len(&self, len: u64) -> Result<(), Error> {
        unimplemented!()
    }

    fn sync_all(&self) -> Result<(), Error> {
        unimplemented!()
    }

    fn sync_data(&self) -> Result<(), Error> {
        unimplemented!()
    }

    fn read_entry(&self) -> Result<String, Error> {
        unimplemented!()
    }

    fn as_any(&self) -> &Any {
        self
    }
}

impl UnixSocketFile {
    pub fn new(socket_type: c_int, protocol: c_int) -> Result<Self, Error> {
        let inner = UnixSocket::new(socket_type, protocol)?;
        Ok(UnixSocketFile { inner: Mutex::new(inner) })
    }

    pub fn bind(&self, path: impl AsRef<str>) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.bind(path)
    }

    pub fn listen(&self) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.listen()
    }

    pub fn accept(&self) -> Result<UnixSocketFile, Error> {
        let mut inner = self.inner.lock().unwrap();
        let new_socket = inner.accept()?;
        Ok(UnixSocketFile { inner: Mutex::new(new_socket) })
    }

    pub fn connect(&self, path: impl AsRef<str>) -> Result<(), Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.connect(path)
    }
}

impl Debug for UnixSocketFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "UnixSocketFile {{ ... }}")
    }
}


pub trait AsUnixSocket {
    fn as_unix_socket(&self) -> Result<&UnixSocketFile, Error>;
}

impl AsUnixSocket for FileRef {
    fn as_unix_socket(&self) -> Result<&UnixSocketFile, Error> {
        self.as_any()
            .downcast_ref::<UnixSocketFile>()
            .ok_or(Error::new(Errno::EBADF, "not a unix socket"))
    }
}


pub struct UnixSocket {
    obj: Option<Arc<UnixSocketObject>>,
    status: Status,
}

enum Status {
    None,
    Listening,
    Connected(Channel),
}

impl UnixSocket {
    /// C/S 1: Create a new unix socket
    pub fn new(socket_type: c_int, protocol: c_int) -> Result<Self, Error> {
        if socket_type == libc::SOCK_STREAM && protocol == 0 {
            Ok(UnixSocket {
                obj: None,
                status: Status::None
            })
        } else {
            errno!(ENOSYS, "unimplemented unix socket type")
        }
    }

    /// Server 2: Bind the socket to a file system path
    pub fn bind(&mut self, path: impl AsRef<str>) -> Result<(), Error> {
        // TODO: check permission
        if self.obj.is_some() {
            return errno!(EINVAL, "The socket is already bound to an address.");
        }
        self.obj = Some(UnixSocketObject::create(path)?);
        Ok(())
    }

    /// Server 3: Listen to a socket
    pub fn listen(&mut self) -> Result<(), Error> {
        self.status = Status::Listening;
        Ok(())
    }

    /// Server 4: Accept a connection on listening. Non-blocking.
    pub fn accept(&mut self) -> Result<UnixSocket, Error> {
        match self.status {
            Status::Listening => {}
            _ => return errno!(EINVAL, "unix socket is not listening"),
        };
        let socket = self.obj.as_mut().unwrap().pop()
            .ok_or(Error::new(EAGAIN, "no connections are present to be accepted"))?;
        Ok(socket)
    }

    /// Client 2: Connect to a path
    pub fn connect(&mut self, path: impl AsRef<str>) -> Result<(), Error> {
        if let Status::Listening = self.status {
            return errno!(EINVAL, "unix socket is listening?");
        }
        let obj = UnixSocketObject::get(path)
            .ok_or(Error::new(EINVAL, "unix socket path not found"))?;
        let (channel1, channel2) = Channel::new_pair();
        self.status = Status::Connected(channel1);
        obj.push(UnixSocket {
            obj: Some(obj.clone()),
            status: Status::Connected(channel2),
        });
        Ok(())
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        if let Status::Connected(channel) = &self.status {
            channel.reader.read(buf)
        } else {
            errno!(EBADF, "UnixSocket is not connected")
        }
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        if let Status::Connected(channel) = &self.status {
            channel.writer.write(buf)
        } else {
            errno!(EBADF, "UnixSocket is not connected")
        }
    }
}

impl Drop for UnixSocket {
    fn drop(&mut self) {
        if let Status::Listening = self.status {
            let path = &self.obj.as_ref().unwrap().path;
            UnixSocketObject::remove(path);
        }
    }
}

pub struct UnixSocketObject {
    path: String,
    accepted_sockets: Mutex<VecDeque<UnixSocket>>,
}

impl UnixSocketObject {
    fn push(&self, unix_socket: UnixSocket) {
        let mut queue = self.accepted_sockets.lock().unwrap();
        queue.push_back(unix_socket);
    }
    fn pop(&self) -> Option<UnixSocket> {
        let mut queue = self.accepted_sockets.lock().unwrap();
        queue.pop_front()
    }
    fn get(path: impl AsRef<str>) -> Option<Arc<Self>> {
        let mut paths = UNIX_SOCKET_OBJS.lock().unwrap();
        paths.get(path.as_ref()).map(|obj| obj.clone())
    }
    fn create(path: impl AsRef<str>) -> Result<Arc<Self>, Error> {
        let mut paths = UNIX_SOCKET_OBJS.lock().unwrap();
        if paths.contains_key(path.as_ref()) {
            return errno!(EADDRINUSE, "unix socket path already exists");
        }
        let obj = Arc::new(UnixSocketObject {
            path: path.as_ref().to_string(),
            accepted_sockets: Mutex::new(VecDeque::new())
        });
        paths.insert(path.as_ref().to_string(), obj.clone());
        Ok(obj)
    }
    fn remove(path: impl AsRef<str>) {
        let mut paths = UNIX_SOCKET_OBJS.lock().unwrap();
        paths.remove(path.as_ref());
    }
}

struct Channel {
    reader: RingBufReader,
    writer: RingBufWriter
}

unsafe impl Send for Channel {}
unsafe impl Sync for Channel {}

impl Channel {
    fn new_pair() -> (Channel, Channel) {
        let buf1 = RingBuf::new(DEFAULT_BUF_SIZE);
        let buf2 = RingBuf::new(DEFAULT_BUF_SIZE);
        let channel1 = Channel {
            reader: buf1.reader,
            writer: buf2.writer,
        };
        let channel2 = Channel {
            reader: buf2.reader,
            writer: buf1.writer,
        };
        (channel1, channel2)
    }
}

pub const DEFAULT_BUF_SIZE: usize = 1 * 1024 * 1024;

lazy_static! {
    static ref UNIX_SOCKET_OBJS: Mutex<BTreeMap<String, Arc<UnixSocketObject>>>
        = Mutex::new(BTreeMap::new());
}
