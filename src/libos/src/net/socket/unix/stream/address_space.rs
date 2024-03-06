use super::endpoint::Endpoint;
use super::endpoint::RelayNotifier;
use super::stream::Listener;
use super::*;
use std::collections::btree_map::BTreeMap;

lazy_static! {
    pub(super) static ref ADDRESS_SPACE: AddressSpace = AddressSpace::new();
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum AddressSpaceKey {
    FileKey(usize),
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

    pub fn add_binder(&self, addr: &UnixAddr) -> Result<()> {
        let key = Self::get_key(addr).ok_or_else(|| errno!(EINVAL, "can't find socket file"))?;
        let mut space = self.get_space(addr)?;
        if space.contains_key(&key) {
            return_errno!(EADDRINUSE, "the addr is already bound");
        } else {
            space.insert(key, None);
            Ok(())
        }
    }

    pub(super) fn add_listener(
        &self,
        addr: &UnixAddr,
        capacity: usize,
        nonblocking: bool,
        notifier: Arc<RelayNotifier>,
    ) -> Result<()> {
        let key = Self::get_key(addr).ok_or_else(|| errno!(EINVAL, "the socket is not bound"))?;
        let mut space = self.get_space(addr)?;

        if let Some(option) = space.get(&key) {
            if option.is_none() {
                space.insert(
                    key,
                    Some(Arc::new(Listener::new(capacity, nonblocking, notifier)?)),
                );
                Ok(())
            } else {
                return_errno!(EINVAL, "the socket is already listened");
            }
        } else {
            return_errno!(EINVAL, "the socket is not bound");
        }
    }

    pub fn resize_listener(&self, addr: &UnixAddr, capacity: usize) -> Result<()> {
        let key = Self::get_key(addr).ok_or_else(|| errno!(EINVAL, "the socket is not bound"))?;
        let mut space = self.get_space(addr)?;

        if let Some(option) = space.get(&key) {
            if let Some(listener) = option {
                listener.resize(capacity);
            } else {
                return_errno!(EINVAL, "the socket is not listening");
            }
            Ok(())
        } else {
            return_errno!(EINVAL, "the socket is not bound");
        }
    }

    pub fn push_incoming(&self, addr: &UnixAddr, sock: Endpoint) -> Result<()> {
        self.get_listener_ref(addr)
            .ok_or_else(|| errno!(ECONNREFUSED, "no one's listening on the remote address"))?
            .push_incoming(sock)
    }

    pub fn pop_incoming(&self, addr: &UnixAddr) -> Result<Endpoint> {
        self.get_listener_ref(addr)
            .ok_or_else(|| errno!(EINVAL, "the socket is not listening"))?
            .pop_incoming()
            .ok_or_else(|| errno!(EAGAIN, "No connection is incoming"))
    }

    pub fn get_listener_ref(&self, addr: &UnixAddr) -> Option<Arc<Listener>> {
        let key = Self::get_key(addr);
        if let Some(key) = key {
            let space = self.get_space(addr).unwrap();
            space.get(&key).map(|x| x.clone()).flatten()
        } else {
            None
        }
    }

    pub fn remove_addr(&self, addr: &UnixAddr) {
        let key = Self::get_key(addr);
        if let Some(key) = key {
            let mut space = self.get_space(addr).unwrap();
            space.remove(&key);
        } else {
            warn!("address space key not exit: {:?}", addr);
        }
    }

    fn get_space(
        &self,
        addr: &UnixAddr,
    ) -> Result<SgxMutexGuard<'_, BTreeMap<AddressSpaceKey, Option<Arc<Listener>>>>> {
        match addr {
            UnixAddr::File(_, _) => Ok(self.file.lock().unwrap()),
            UnixAddr::Abstract(_) => Ok(self.abstr.lock().unwrap()),
            UnixAddr::Unnamed => return_errno!(EINVAL, "can't get path name for unnamed socket"),
        }
    }

    fn get_key(addr: &UnixAddr) -> Option<AddressSpaceKey> {
        trace!("addr = {:?}", addr);
        match addr {
            UnixAddr::File(inode_num, unix_path) if inode_num.is_some() => {
                Some(AddressSpaceKey::from_inode(inode_num.unwrap()))
            }
            UnixAddr::File(_, unix_path) => {
                let inode = {
                    let file_path = unix_path.absolute();
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
                addr.path_str().unwrap().to_string(),
            )),
            UnixAddr::Unnamed => None,
        }
    }
}
