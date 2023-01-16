use async_io::fs::{FileMode, FileType};
use async_recursion::async_recursion;
use async_rt::sync::{RwLock as AsyncRwLock, RwLockWriteGuard as AsyncRwLockWriteGuard};
use async_rt::wait::{Waiter, WaiterQueue};
use keyable_arc::KeyableArc;
use lru::LruCache;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::time::Duration;

use crate::fs::AsyncFileSystem;
use crate::inode::AsyncInode;
use crate::prelude::*;

lazy_static! {
    static ref DCACHE: DCache = DCache::new(256);
}

/// The DCache is a memory cache for Dentry objects.
pub struct DCache(Arc<Inner>);

struct Inner {
    // A normal cache to store the valid dentries.
    // This cache will evict the unused dentries to the LRU cache if size exceeds limit.
    cache: Mutex<HashMap<KeyableArc<Dentry>, ()>>,
    // To reduce lock contentions, maintain a counter for the size.
    cache_size: AtomicUsize,
    // The size limit of normal cache.
    cache_limit: usize,
    // A LRU cache to store the invalid dentries and evicted dentries from normal cache.
    // This cache will pop the LRU dentries if size exceeds limit.
    lru_cache: Mutex<LruCache<KeyableArc<Dentry>, ()>>,
    // To reduce lock contentions, maintain a counter for the size.
    lru_cache_size: AtomicUsize,
    // The size limit of LRU cache.
    lru_cache_limit: usize,
    cleaner_wq: WaiterQueue,
    is_dropped: AtomicBool,
}

impl Inner {
    fn clean(&self) {
        // Shrink the normal cache, evict unused dentries to LRU cache.
        let cache_size = self.cache_size.load(Ordering::Relaxed);
        if cache_size > self.cache_limit {
            let mut num_evicted = 0;
            self.cache.lock().unwrap().retain(|dentry, _| {
                let dentry: &Arc<Dentry> = unsafe { core::mem::transmute(dentry) };
                if Arc::strong_count(dentry) == 1 {
                    dentry.set_evited();
                    debug!("evict Dentry = {:?} to LRU dcache", dentry);
                    self.lru_cache
                        .lock()
                        .unwrap()
                        .push(dentry.clone().into(), ());
                    num_evicted += 1;
                    false
                } else {
                    true
                }
            });
            if num_evicted > 0 {
                self.cache_size.fetch_sub(num_evicted, Ordering::Release);
                self.lru_cache_size
                    .fetch_add(num_evicted, Ordering::Release);
            }
        }

        // Shrink the LRU cache.
        let lru_cache_size = self.lru_cache_size.load(Ordering::Relaxed);
        if lru_cache_size > self.lru_cache_limit {
            let mut lru_cache = self.lru_cache.lock().unwrap();
            let shrink_len = lru_cache.len() / 2;
            for _ in 0..shrink_len {
                lru_cache.pop_lru();
            }
            self.lru_cache_size.fetch_sub(shrink_len, Ordering::Release);
        }
    }
}

impl DCache {
    /// Create a new DCache with max size limit.
    pub fn new(cache_limit: usize) -> Self {
        let inner = Inner {
            cache: Mutex::new(HashMap::new()),
            cache_size: AtomicUsize::new(0),
            cache_limit: cache_limit,
            lru_cache: Mutex::new(LruCache::unbounded()),
            lru_cache_size: AtomicUsize::new(0),
            lru_cache_limit: cache_limit,
            cleaner_wq: WaiterQueue::new(),
            is_dropped: AtomicBool::new(false),
        };
        let new_self = Self(Arc::new(inner));
        new_self.spawn_clean_task();
        new_self
    }

    /// Spawn a clean task.
    ///
    /// The task cleans the cached dentries on demond.
    fn spawn_clean_task(&self) {
        const AUTO_FLUSH_PERIOD: Duration = Duration::from_secs(5);
        let this = self.0.clone();
        // Spawn the clean task
        async_rt::task::spawn(async move {
            let mut waiter = Waiter::new();
            this.cleaner_wq.enqueue(&mut waiter);
            loop {
                // If is dropped, then the task should exit
                if this.is_dropped.load(Ordering::Relaxed) {
                    break;
                }

                // Wait until being notified or timeout
                let mut timeout = AUTO_FLUSH_PERIOD;
                let _ = waiter.wait_timeout(Some(&mut timeout)).await;

                // Do work
                this.clean();
            }
            this.cleaner_wq.dequeue(&mut waiter);
        });
    }

    /// Insert a dentry.
    pub fn insert(&self, dentry: KeyableArc<Dentry>) {
        if dentry.is_invalid() || dentry.is_evicted() {
            debug!("insert Dentry = {:?} into LRU dcache", dentry);
            self.0.lru_cache.lock().unwrap().push(dentry, ());
            self.0.lru_cache_size.fetch_add(1, Ordering::Release);
        } else {
            let inode = dentry.inode();
            if inode.is_dentry_cacheable() {
                debug!("insert Dentry = {:?} into dcache", dentry);
                self.0.cache.lock().unwrap().insert(dentry, ());
                self.0.cache_size.fetch_add(1, Ordering::Release);
            }
        }
    }

    /// Remove a dentry.
    pub fn remove(&self, dentry: &KeyableArc<Dentry>) {
        if dentry.is_invalid() || dentry.is_evicted() {
            debug!("remove Dentry = {:?} from LRU dcache", dentry);
            self.0.lru_cache.lock().unwrap().pop(dentry);
            self.0.lru_cache_size.fetch_sub(1, Ordering::Release);
        } else {
            debug!("remove Dentry = {:?} from dcache", dentry);
            self.0.cache.lock().unwrap().remove(dentry);
            self.0.cache_size.fetch_sub(1, Ordering::Release);
        }
    }

    /// Update the LRU.
    fn update_lru(&self, dentry: &KeyableArc<Dentry>) {
        self.0.lru_cache.lock().unwrap().get(dentry);
    }

    /// Evict the dentry to LRU.
    fn evict_to_lru(&self, dentry: &KeyableArc<Dentry>) {
        if let Some((dentry, _)) = self.0.cache.lock().unwrap().remove_entry(dentry) {
            debug!("evict Dentry = {:?} to LRU dcache", dentry);
            self.0.cache_size.fetch_sub(1, Ordering::Release);
            self.0.lru_cache.lock().unwrap().push(dentry, ());
            self.0.lru_cache_size.fetch_add(1, Ordering::Release);
        }
    }
}

impl Drop for DCache {
    fn drop(&mut self) {
        self.0.is_dropped.store(true, Ordering::Relaxed);
        self.0.cleaner_wq.wake_all();
    }
}

/// The Dentry is used to speed up the pathname lookup.
pub struct Dentry {
    /// Underlying AsyncInode
    /// For negative dentry, this field is none.
    inode: RwLock<Option<Arc<dyn AsyncInode>>>,
    /// Flags, for efficient manipulation.
    flags: AtomicU8,
    /// Child dentry
    children: AsyncRwLock<Children>,
    /// Name and Parent dentry
    name_and_parent: RwLock<(String, Option<Arc<Dentry>>)>,
    /// Pointer to self
    this: Weak<Self>,
}

impl Dentry {
    /// Create a new root dentry tree with root inode.
    pub fn new_root(root_inode: Arc<dyn AsyncInode>) -> Arc<Self> {
        let root = Self::new("/", Some(root_inode), None);
        DCACHE.insert(root.clone().into());
        root
    }

    /// The internal constructor.
    fn new(
        name: &str,
        inode: Option<Arc<dyn AsyncInode>>,
        parent: Option<Arc<Dentry>>,
    ) -> Arc<Self> {
        Arc::new_cyclic(|weak_self| Self {
            flags: {
                let flags = if inode.is_none() {
                    DentryFlags::NEGATIVE
                } else {
                    DentryFlags::empty()
                };
                AtomicU8::new(flags.bits())
            },
            inode: RwLock::new(inode),
            children: AsyncRwLock::new(Children::new()),
            name_and_parent: RwLock::new((name.to_owned(), parent)),
            this: weak_self.clone(),
        })
    }

    fn flags(&self) -> DentryFlags {
        let flags = self.flags.load(Ordering::Relaxed);
        DentryFlags::from_bits(flags).unwrap()
    }

    /// Whether is an invalid dentry.
    fn is_invalid(&self) -> bool {
        self.is_negative() || self.is_deleted()
    }

    /// Whether is negative dentry.
    fn is_negative(&self) -> bool {
        self.flags().contains(DentryFlags::NEGATIVE)
    }

    /// Whether is deleted by unlink.
    fn is_deleted(&self) -> bool {
        self.flags().contains(DentryFlags::DELETED)
    }

    /// Set deleted flags.
    fn set_deleted(&self) {
        self.flags
            .fetch_or((DentryFlags::DELETED).bits(), Ordering::Release);
    }

    /// Whether is evicted because of shrinking cache size.
    fn is_evicted(&self) -> bool {
        self.flags().contains(DentryFlags::EVICTED)
    }

    /// Set evivted flags.
    fn set_evited(&self) {
        self.flags
            .fetch_or((DentryFlags::EVICTED).bits(), Ordering::Release);
    }

    /// Return the name.
    pub fn name(&self) -> String {
        self.name_and_parent.read().unwrap().0.clone()
    }

    /// Return the parent dentry if exists.
    pub fn parent(&self) -> Option<Arc<Dentry>> {
        self.name_and_parent.read().unwrap().1.clone()
    }

    /// Set the name and parent, used in rename.
    fn set_name_and_parent(&self, name: &str, parent: Option<Arc<Dentry>>) {
        let mut name_and_parent = self.name_and_parent.write().unwrap();
        name_and_parent.0 = name.to_owned();
        name_and_parent.1 = parent;
    }

    /// Return strong pointer to self.
    fn this(&self) -> Arc<Dentry> {
        self.this.upgrade().unwrap()
    }

    /// Return the underlying inode for non-negative dentry.
    pub fn inode(&self) -> Arc<dyn AsyncInode> {
        debug_assert!(!self.is_negative());
        self.inode.read().unwrap().as_ref().unwrap().clone()
    }

    /// Set the underlying inode for non-negative dentry.
    fn set_inode(&self, inode: Arc<dyn AsyncInode>) {
        debug_assert!(!self.is_negative());
        *self.inode.write().unwrap() = Some(inode);
    }

    /// Create a child dentry.
    pub async fn create(&self, name: &str, type_: FileType, mode: FileMode) -> Result<Arc<Self>> {
        let self_inode = self.inode();
        if self_inode.metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "self is not dir");
        }

        let mut children_mut = self.children.write().await;
        if children_mut.find_valid(name).is_some() {
            return_errno!(EEXIST, "");
        }

        let child = {
            let child_inode = self_inode.create(name, type_, mode.bits()).await?;
            Dentry::new(name, Some(child_inode), Some(self.this()))
        };
        children_mut.insert(name.to_owned(), child.clone());

        Ok(child)
    }

    /// Lookup a dentry by name.
    pub async fn find(&self, name: &str) -> Result<Arc<Dentry>> {
        let self_inode = self.inode();
        if self_inode.metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "self is not dir");
        }

        let dentry = match name {
            "." => self.this(),
            ".." => self.parent().unwrap_or(self.this()),
            name => {
                let mut children_mut = self.children.write().await;
                if let Some(dentry) = children_mut.find(name) {
                    dentry.clone()
                } else {
                    let child = {
                        let child_inode = self_inode.find(name).await.ok();
                        Dentry::new(name, child_inode, Some(self.this()))
                    };
                    children_mut.insert(name.to_owned(), child.clone());
                    child
                }
            }
        };

        if dentry.is_invalid() {
            return_errno!(ENOENT, "");
        }

        Ok(dentry)
    }

    /// Link a dentry.
    ///
    /// It will create a new-named dentry with same inode.
    pub async fn link(&self, new_name: &str, target: &Arc<Dentry>) -> Result<()> {
        let self_inode = self.inode();
        if self_inode.metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "self is not dir");
        }

        let mut children_mut = self.children.write().await;
        if children_mut.find_valid(new_name).is_some() {
            return_errno!(EEXIST, "");
        }

        let target_inode = target.inode();
        self_inode.link(new_name, &target_inode).await?;
        let new_dentry = Dentry::new(new_name, Some(target_inode), Some(self.this()));
        children_mut.insert(new_name.to_owned(), new_dentry);

        Ok(())
    }

    /// Unlink a child dentry.
    pub async fn unlink(&self, name: &str) -> Result<()> {
        let self_inode = self.inode();
        if self_inode.metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "self is not dir");
        }

        let mut children_mut = self.children.write().await;
        self_inode.unlink(name).await?;
        if let Some(child) = children_mut.find(name) {
            child.set_deleted();
            DCACHE.evict_to_lru(&child.into());
        }

        Ok(())
    }

    /// Rename a child dentry to target.
    pub async fn move_(&self, old_name: &str, target: &Arc<Dentry>, new_name: &str) -> Result<()> {
        if old_name == "." || old_name == ".." || new_name == "." || new_name == ".." {
            return_errno!(EISDIR, "oldpath or newpath is a directory");
        }

        // Self and target are same Dentry, just modify name
        if Arc::ptr_eq(&self.this(), target) {
            let self_inode = self.inode();
            if self_inode.metadata().await?.type_ != FileType::Dir {
                return_errno!(ENOTDIR, "self is not dir");
            }
            if old_name == new_name {
                return Ok(());
            }
            let mut children_mut = self.children.write().await;
            let dentry = if let Some(dentry) = children_mut.find(old_name) {
                dentry.clone()
            } else {
                let child_inode = self_inode.find(old_name).await.ok();
                let dentry = Dentry::new(old_name, child_inode, Some(self.this()));
                children_mut.insert(old_name.to_owned(), dentry.clone());
                dentry
            };
            if dentry.is_invalid() {
                return_errno!(ENOENT, "");
            }
            self_inode.move_(old_name, &self_inode, new_name).await?;
            children_mut.remove(old_name);
            dentry.set_name_and_parent(new_name, Some(self.this()));
            children_mut.insert(new_name.to_owned(), dentry);
        } else {
            let self_inode = self.inode();
            let target_inode = target.inode();
            // Self and target are different Dentry
            if self_inode.metadata().await?.type_ != FileType::Dir
                || target_inode.metadata().await?.type_ != FileType::Dir
            {
                return_errno!(ENOTDIR, "self or target is not dir");
            }
            let (mut self_children, mut target_children) =
                write_lock_two_children(&self, &target).await;
            let dentry = if let Some(dentry) = self_children.find(old_name) {
                dentry.clone()
            } else {
                let inode = self_inode.find(old_name).await.ok();
                let dentry = Dentry::new(old_name, inode, Some(self.this()));
                self_children.insert(old_name.to_owned(), dentry.clone());
                dentry
            };
            if dentry.is_invalid() {
                return_errno!(ENOENT, "");
            }
            self_inode.move_(old_name, &target_inode, new_name).await?;
            self_children.remove(old_name);
            dentry.set_name_and_parent(new_name, Some(target.this()));
            target_children.insert(new_name.to_owned(), dentry);
        }

        Ok(())
    }

    /// Mount an FS on the dentry.
    pub async fn mount(&self, fs: Arc<dyn AsyncFileSystem>) -> Result<()> {
        let parent = self.parent().ok_or(errno!(EPERM, "cannot mount root"))?;
        let parent_inode = parent.inode();
        let self_inode = self.inode();

        if self_inode.metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "self is not dir");
        }

        let mut children_mut = self.children.write().await;
        self_inode.mount(fs).await?;
        self.set_inode(parent_inode.find(&self.name()).await?);
        children_mut.mount();

        Ok(())
    }

    /// Umount an FS from the dentry.
    pub async fn umount(&self) -> Result<()> {
        let parent = self.parent().ok_or(errno!(EPERM, "cannot umount root"))?;
        let parent_inode = parent.inode();
        let self_inode = self.inode();

        if self_inode.metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "self is not dir");
        }

        let mut children_mut = self.children.write().await;
        self_inode.umount().await?;
        self.set_inode(parent_inode.find(&self.name()).await?);
        children_mut.umount().await;

        Ok(())
    }

    /// Return absolute path of the dentry.
    pub fn abs_path(&self) -> String {
        let mut path = self.name();
        let mut dentry = self.this();

        loop {
            match dentry.parent() {
                None => break,
                Some(parent_dentry) => {
                    path = {
                        let parent_name = parent_dentry.name();
                        if parent_name != "/" {
                            parent_name + "/" + &path
                        } else {
                            parent_name + &path
                        }
                    };
                    dentry = parent_dentry;
                }
            }
        }

        debug_assert!(path.starts_with("/"));
        path
    }
}

impl std::fmt::Debug for Dentry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dentry")
            .field("path", &self.abs_path())
            .field("flags", &self.flags())
            .finish()
    }
}

async fn write_lock_two_children<'a>(
    this: &'a Dentry,
    other: &'a Dentry,
) -> (
    AsyncRwLockWriteGuard<'a, Children>,
    AsyncRwLockWriteGuard<'a, Children>,
) {
    let this_ptr = Arc::as_ptr(&this.this());
    let other_ptr = Arc::as_ptr(&other.this());
    if this_ptr < other_ptr {
        let this = this.children.write().await;
        let other = other.children.write().await;
        (this, other)
    } else {
        let other = other.children.write().await;
        let this = this.children.write().await;
        (this, other)
    }
}

bitflags::bitflags! {
    struct DentryFlags : u8 {
        /// Whether is not associated with a Inode
        const NEGATIVE = 0x01;
        /// Whether is deleted by unlink
        const DELETED = 0x02;
        /// Whether is evicted because of shrinking cache size
        const EVICTED = 0x04;
    }
}

struct Children {
    maps: VecDeque<HashMap<String, Weak<Dentry>>>,
}

impl Children {
    pub fn new() -> Self {
        Self {
            maps: VecDeque::with_capacity(1),
        }
    }

    /// Insert the child dentry into map.
    ///
    /// The dentry may be inserted into cache.
    pub fn insert(&mut self, name: String, dentry: Arc<Dentry>) {
        if self.maps.is_empty() {
            self.maps.push_front(HashMap::new());
        }

        if let Some(old) = self.maps[0].insert(name, Arc::downgrade(&dentry)) {
            // Remove the old from cache
            if let Some(old) = old.upgrade() {
                let old: KeyableArc<Dentry> = old.into();
                DCACHE.remove(&old);
            }
        }

        DCACHE.insert(dentry.into());
    }

    /// Remove and return the child dentry by name.
    ///
    /// This operation does not affect the cache.
    pub fn remove(&mut self, name: &str) -> Option<Arc<Dentry>> {
        if self.maps.is_empty() {
            return None;
        }
        self.maps[0].remove(name).and_then(|d| d.upgrade())
    }

    /// Mount a new map at the upper layer.
    pub fn mount(&mut self) {
        self.maps.push_front(HashMap::new());
    }

    /// Unmount the upper layer map.
    ///
    /// This operation will remove the child dentries from cache recursively.
    #[async_recursion(?Send)]
    pub async fn umount(&mut self) {
        if let Some(map) = self.maps.pop_front() {
            for dentry in map.values() {
                if let Some(dentry) = dentry.upgrade() {
                    let dentry: KeyableArc<Dentry> = dentry.into();
                    DCACHE.remove(&dentry);
                    let mut children_mut = dentry.children.write().await;
                    children_mut.umount().await;
                }
            }
        }
    }

    /// Return the child dentry by name.
    pub fn find(&mut self, name: &str) -> Option<Arc<Dentry>> {
        if self.maps.is_empty() {
            return None;
        }

        if let Some(dentry) = self.maps[0].get(name) {
            match dentry.upgrade() {
                Some(dentry) => {
                    let dentry_ref: &KeyableArc<Dentry> = unsafe { core::mem::transmute(&dentry) };
                    if dentry_ref.is_invalid() || dentry_ref.is_evicted() {
                        DCACHE.update_lru(dentry_ref);
                    }
                    Some(dentry)
                }
                None => {
                    self.maps[0].remove(name);
                    None
                }
            }
        } else {
            None
        }
    }

    /// Return only the valid child dentry by name.
    pub fn find_valid(&mut self, name: &str) -> Option<Arc<Dentry>> {
        if let Some(dentry) = self.find(name) {
            if dentry.is_invalid() {
                None
            } else {
                Some(dentry)
            }
        } else {
            None
        }
    }
}
