use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use super::*;

/// A unique fd from the host OS.
///
/// The uniqueness property is important both
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct HostFd(FileDesc);

impl HostFd {
    pub fn new(host_fd: FileDesc) -> Self {
        HOST_FD_REGISTRY.lock().unwrap().register(host_fd).unwrap();
        Self(host_fd)
    }

    pub fn to_raw(&self) -> FileDesc {
        self.0
    }
}

impl Drop for HostFd {
    fn drop(&mut self) {
        HOST_FD_REGISTRY
            .lock()
            .unwrap()
            .unregister(self.to_raw())
            .unwrap();
    }
}

lazy_static! {
    static ref HOST_FD_REGISTRY: SgxMutex<HostFdRegistry> =
        { SgxMutex::new(HostFdRegistry::new()) };
}

/// A registery for host fds to ensure that they are unique.
struct HostFdRegistry {
    set: HashSet<FileDesc>,
}

impl HostFdRegistry {
    pub fn new() -> Self {
        Self {
            set: HashSet::new(),
        }
    }

    pub fn register(&mut self, host_fd: FileDesc) -> Result<()> {
        let new_val = self.set.insert(host_fd);
        if !new_val {
            return_errno!(EEXIST, "host fd has been registered");
        }
        Ok(())
    }

    pub fn unregister(&mut self, host_fd: FileDesc) -> Result<()> {
        let existing = self.set.remove(&host_fd);
        if !existing {
            return_errno!(ENOENT, "host fd has NOT been registered");
        }
        Ok(())
    }
}
