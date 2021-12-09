use super::sock_end::SockEnd as Endpoint;
use super::stream::Listener;
use super::*;
use crate::fs::FsPath;
use std::collections::btree_map::BTreeMap;
use std::convert::TryFrom;

lazy_static! {
    pub(super) static ref ADDRESS_SPACE: AddressSpace = AddressSpace::new();
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AddressSpaceKey {
    FileKey(usize), // inode number
    AbstrKey(String),
}

impl AddressSpaceKey {
    pub fn from_inode(inode: usize) -> Self {
        AddressSpaceKey::FileKey(inode)
    }

    pub fn from_path(path: String) -> Self {
        AddressSpaceKey::AbstrKey(path)
    }
}

pub struct AddressSpace {
    // For "file", use inode number as "key" instead of path string so that listeners can still
    // be reached even if the socket file is moved or renamed.
    file: SgxMutex<BTreeMap<AddressSpaceKey, Option<Arc<Listener>>>>,
    abstr: SgxMutex<BTreeMap<AddressSpaceKey, Option<Arc<Listener>>>>,
}

impl AddressSpace {
    pub fn new() -> Self {
        Self {
            file: SgxMutex::new(BTreeMap::new()),
            abstr: SgxMutex::new(BTreeMap::new()),
        }
    }

    pub fn add_binder(&self, addr: &TrustedAddr) -> Result<()> {
        let key = Self::get_key(addr).ok_or_else(|| errno!(EINVAL, "can't find socket file"))?;
        let mut space = self.get_space(addr);
        if space.contains_key(&key) {
            return_errno!(EADDRINUSE, "the addr is already bound");
        } else {
            space.insert(key, None);
            Ok(())
        }
    }

    pub fn add_listener(
        &self,
        addr: &TrustedAddr,
        capacity: usize,
        nonblocking: bool,
    ) -> Result<()> {
        let key = Self::get_key(addr).ok_or_else(|| errno!(EINVAL, "the socket is not bound"))?;
        let mut space = self.get_space(addr);

        if let Some(option) = space.get(&key) {
            if option.is_none() {
                space.insert(key, Some(Arc::new(Listener::new(capacity, nonblocking)?)));
                Ok(())
            } else {
                return_errno!(EINVAL, "the socket is already listened");
            }
        } else {
            return_errno!(EINVAL, "the socket is not bound");
        }
    }

    pub fn push_incoming(&self, addr: &TrustedAddr, sock: Endpoint) -> Result<()> {
        self.get_listener_ref(addr)
            .ok_or_else(|| errno!(ECONNREFUSED, "no one's listening on the remote address"))?
            .push_incoming(sock)
    }

    pub async fn pop_incoming(&self, addr: &TrustedAddr) -> Result<Endpoint> {
        self.get_listener_ref(addr)
            .ok_or_else(|| errno!(EINVAL, "the socket is not listening"))?
            .pop_incoming()
            .await
    }

    pub fn get_listener_ref(&self, addr: &TrustedAddr) -> Option<Arc<Listener>> {
        let key = Self::get_key(addr);
        trace!("get listener key = {:?}", key);
        if let Some(key) = key {
            let space = self.get_space(addr);
            space.get(&key).map(|x| x.clone()).flatten()
        } else {
            None
        }
    }

    pub fn remove_addr(&self, addr: &TrustedAddr) {
        let key = Self::get_key(addr);
        if let Some(key) = key {
            let mut space = self.get_space(addr);
            space.remove(&key);
        } else {
            warn!("address space key not exit: {:?}", addr);
        }
    }

    fn get_space(
        &self,
        addr: &TrustedAddr,
    ) -> SgxMutexGuard<'_, BTreeMap<AddressSpaceKey, Option<Arc<Listener>>>> {
        match *addr.inner() {
            UnixAddr::Pathname(_) => self.file.lock().unwrap(),
            UnixAddr::Abstract(_) => self.abstr.lock().unwrap(),
            _ => unimplemented!(),
        }
    }

    fn get_key(addr: &TrustedAddr) -> Option<AddressSpaceKey> {
        trace!("addr = {:?}", addr);
        if let Some(inode_num) = addr.inode() {
            Some(AddressSpaceKey::from_inode(inode_num))
        } else {
            match &*addr.inner() {
                UnixAddr::Pathname(unix_path) => {
                    let inode = {
                        let file_path = FsPath::try_from(unix_path.as_ref()).unwrap();
                        let current = current!();
                        let fs = current.fs().read().unwrap();
                        fs.lookup_inode(&file_path)
                    };
                    if let Ok(inode) = inode {
                        Some(AddressSpaceKey::from_inode(inode.metadata().unwrap().inode))
                    } else {
                        None
                    }
                }
                UnixAddr::Abstract(path) => Some(AddressSpaceKey::from_path(
                    String::from_utf8_lossy(&path).to_string(),
                )),
                _ => unimplemented!(),
            }
        }
    }
}
