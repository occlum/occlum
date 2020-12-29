use super::endpoint::Endpoint;
use super::stream::Listener;
use super::*;
use std::collections::btree_map::BTreeMap;

lazy_static! {
    pub(super) static ref ADDRESS_SPACE: AddressSpace = AddressSpace::new();
}

pub struct AddressSpace {
    file: SgxMutex<BTreeMap<String, Option<Arc<Listener>>>>,
    abstr: SgxMutex<BTreeMap<String, Option<Arc<Listener>>>>,
}

impl AddressSpace {
    pub fn new() -> Self {
        Self {
            file: SgxMutex::new(BTreeMap::new()),
            abstr: SgxMutex::new(BTreeMap::new()),
        }
    }

    pub fn add_binder(&self, addr: &Addr) -> Result<()> {
        let key = Self::get_key(addr);
        let mut space = self.get_space(addr);
        if space.contains_key(&key) {
            return_errno!(EADDRINUSE, "the addr is already bound");
        } else {
            space.insert(key, None);
            Ok(())
        }
    }

    pub fn add_listener(&self, addr: &Addr, capacity: usize) -> Result<()> {
        let key = Self::get_key(addr);
        let mut space = self.get_space(addr);

        if let Some(option) = space.get(&key) {
            if let Some(listener) = option {
                let listener = listener.clone();
                let new_listener = Listener::new(capacity)?;
                for i in 0..std::cmp::min(listener.remaining(), capacity) {
                    new_listener.push_incoming(listener.pop_incoming().unwrap());
                }
                space.insert(key, Some(Arc::new(new_listener)));
                /// shutdown the old listener in case it is still being used
                /// by especially blocking accept
                listener.shutdown();
            } else {
                space.insert(key, Some(Arc::new(Listener::new(capacity)?)));
            }
            Ok(())
        } else {
            return_errno!(EINVAL, "the socket is not bound");
        }
    }

    pub fn push_incoming(&self, addr: &Addr, sock: Endpoint) -> Result<()> {
        self.get_listener_ref(addr)
            .ok_or_else(|| errno!(ECONNREFUSED, "no one's listening on the remote address"))?
            .push_incoming(sock);
        Ok(())
    }

    pub fn pop_incoming(&self, addr: &Addr) -> Result<Endpoint> {
        self.get_listener_ref(addr)
            .ok_or_else(|| errno!(EINVAL, "the socket is not listening"))?
            .pop_incoming()
            .ok_or_else(|| errno!(EAGAIN, "No connection is incoming"))
    }

    pub fn get_listener_ref(&self, addr: &Addr) -> Option<Arc<Listener>> {
        let key = Self::get_key(addr);
        let space = self.get_space(addr);
        space.get(&key).map(|x| x.clone()).flatten()
    }

    pub fn remove_addr(&self, addr: &Addr) {
        let key = Self::get_key(addr);
        let mut space = self.get_space(addr);
        space.remove(&key);
    }

    fn get_space(&self, addr: &Addr) -> SgxMutexGuard<'_, BTreeMap<String, Option<Arc<Listener>>>> {
        match addr {
            Addr::File(unix_path) => self.file.lock().unwrap(),
            Addr::Abstract(path) => self.abstr.lock().unwrap(),
        }
    }

    fn get_key(addr: &Addr) -> String {
        match addr {
            Addr::File(unix_path) => unix_path.absolute(),
            Addr::Abstract(path) => addr.path_str().to_string(),
        }
    }
}
