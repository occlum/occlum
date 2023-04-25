use super::*;
use alloc::sync::{Arc, Weak};
use async_trait::async_trait;

use crate::process::pid_t;
use crate::process::table::get_all_processes;

use self::cpuinfo::CpuInfoINode;
use self::meminfo::MemInfoINode;
use self::pid::LockedPidDirINode;
use self::proc_inode::{Dir, DirProcINode, File, ProcINode, SymLink};
use self::self_::SelfSymINode;
use self::stat::StatINode;

mod cpuinfo;
mod meminfo;
mod pid;
mod proc_inode;
mod self_;
mod stat;

// Same with the procfs on Linux
const PROC_SUPER_MAGIC: usize = 0x9fa0;

// Use the same inode number for all the inodes in procfs, the value is
// arbitrarily chosen, and it should not be zero.
// TODO: Assign different inode numbers for different inodes
pub const PROC_INO: usize = 0x63fd_40e5;

/// Proc file system
pub struct ProcFS {
    root: Arc<Dir<LockedProcRootINode>>,
    self_ref: Weak<ProcFS>,
}

#[async_trait]
impl AsyncFileSystem for ProcFS {
    async fn sync(&self) -> Result<()> {
        Ok(())
    }

    async fn root_inode(&self) -> Arc<dyn AsyncInode> {
        Arc::clone(&self.root) as _
    }

    async fn info(&self) -> FsInfo {
        FsInfo {
            magic: PROC_SUPER_MAGIC,
            bsize: 4096,
            frsize: 4096,
            blocks: 0,
            bfree: 0,
            bavail: 0,
            files: 0,
            ffree: 0,
            namemax: 255,
        }
    }
}

impl ProcFS {
    /// Create a new `ProcFS`
    pub fn new() -> Arc<ProcFS> {
        let fs = {
            let root = Arc::new(Dir::new(LockedProcRootINode(RwLock::new(ProcRootINode {
                non_volatile_entries: HashMap::new(),
                this: Weak::default(),
            }))));
            ProcFS {
                root,
                self_ref: Weak::default(),
            }
            .wrap()
        };
        fs.root.inner().init(&fs);
        fs
    }

    /// Wrap pure `ProcFS` with Arc
    /// Used in constructors
    fn wrap(self) -> Arc<ProcFS> {
        let fs = Arc::new(self);
        let weak = Arc::downgrade(&fs);
        let ptr = Arc::into_raw(fs) as *mut ProcFS;
        unsafe {
            (*ptr).self_ref = weak;
        }
        unsafe { Arc::from_raw(ptr) }
    }
}

struct LockedProcRootINode(RwLock<ProcRootINode>);

struct ProcRootINode {
    non_volatile_entries: HashMap<String, Arc<dyn AsyncInode>>,
    this: Weak<Dir<LockedProcRootINode>>,
}

impl LockedProcRootINode {
    fn init(&self, fs: &Arc<ProcFS>) {
        let mut file = self.0.write().unwrap();
        file.this = Arc::downgrade(&fs.root);
        // TODO: Add more entries for root.
        // All [pid] entries are lazy-initialized at the find() step.
        let cpuinfo_inode = CpuInfoINode::new();
        file.non_volatile_entries
            .insert(String::from("cpuinfo"), cpuinfo_inode);
        let meminfo_inode = MemInfoINode::new();
        file.non_volatile_entries
            .insert(String::from("meminfo"), meminfo_inode);
        let self_inode = SelfSymINode::new();
        file.non_volatile_entries
            .insert(String::from("self"), self_inode);
        let stat_inode = StatINode::new();
        file.non_volatile_entries
            .insert(String::from("stat"), stat_inode);
    }
}

#[async_trait]
impl DirProcINode for LockedProcRootINode {
    async fn find(&self, name: &str) -> Result<Arc<dyn AsyncInode>> {
        let file = self.0.read().unwrap();
        if name == "." {
            return Ok(file.this.upgrade().unwrap());
        }
        if name == ".." {
            return Ok(file.this.upgrade().unwrap());
        }

        if let Ok(pid) = name.parse::<pid_t>() {
            let pid_inode = LockedPidDirINode::new(pid, file.this.upgrade().unwrap())?;
            Ok(pid_inode)
        } else if let Some(inode) = file.non_volatile_entries.get(name) {
            Ok(Arc::clone(inode))
        } else {
            return_errno!(ENOENT, "");
        }
    }

    async fn iterate_entries(&self, mut ctx: &mut DirentWriterContext) -> Result<usize> {
        let file = self.0.read().unwrap();
        let idx = ctx.pos();

        // Write first two special entries
        if idx == 0 {
            let this_inode = file.this.upgrade().unwrap();
            write_inode_entry!(&mut ctx, ".", &this_inode);
        }
        if idx <= 1 {
            let parent_inode = file.this.upgrade().unwrap();
            write_inode_entry!(&mut ctx, "..", &parent_inode);
        }

        // Write the non volatile entries
        let skipped = if idx < 2 { 0 } else { idx - 2 };
        for (name, inode) in file.non_volatile_entries.iter().skip(skipped) {
            write_inode_entry!(&mut ctx, name, inode);
        }

        // Write the pid entries
        let skipped = {
            let prior_entries_len = 2 + file.non_volatile_entries.len();
            if idx < prior_entries_len {
                0
            } else {
                idx - prior_entries_len
            }
        };
        for process in get_all_processes().iter().skip(skipped) {
            write_entry!(
                &mut ctx,
                &process.pid().to_string(),
                PROC_INO,
                FileType::Dir
            );
        }

        Ok(ctx.written_len())
    }
}
