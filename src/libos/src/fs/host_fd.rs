use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use super::*;

/// A unique fd from the host OS.
///
/// There are two benefits of using `HostFd` instead of `FileDesc`.
///
/// 1. Uniqueness. Each instance of `HostFd` is guaranteed to have a different
/// value. The uniqueness property makes it possible to use `HostFd` as keys of
/// a hash table.
///
/// 2. Resource Acquisition Is Initialization (RAII). The acquisition and release
/// of the host resource represented by a host fd is bound to the lifetime
/// of the corresponding instance of `HostFd`. This makes resource management
/// simpler and more robust.
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
        let raw_fd = self.to_raw();
        HOST_FD_REGISTRY.lock().unwrap().unregister(raw_fd).unwrap();
        // Note that close MUST be done after unregistering
        unsafe {
            libc::ocall::close(raw_fd as i32);
        }
    }
}

lazy_static! {
    static ref HOST_FD_REGISTRY: SgxMutex<HostFdRegistry> =
        { SgxMutex::new(HostFdRegistry::new()) };
}

/// A registry for host fds to ensure that they are unique.
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
