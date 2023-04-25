use super::*;
use crate::process::table::get_process;
use crate::process::{ProcessRef, ProcessStatus};

use self::cmdline::ProcCmdlineINode;
use self::comm::ProcCommINode;
use self::cwd::ProcCwdSymINode;
use self::exe::ProcExeSymINode;
use self::fd::LockedProcFdDirINode;
use self::maps::ProcMapsINode;
use self::root::ProcRootSymINode;
use self::stat::ProcStatINode;

mod cmdline;
mod comm;
mod cwd;
mod exe;
mod fd;
mod maps;
mod root;
mod stat;

pub struct LockedPidDirINode(RwLock<PidDirINode>);

struct PidDirINode {
    process_ref: ProcessRef,
    this: Weak<Dir<LockedPidDirINode>>,
    parent: Arc<dyn AsyncInode>,
    entries: HashMap<String, Arc<dyn AsyncInode>>,
}

impl LockedPidDirINode {
    pub fn new(pid: pid_t, parent: Arc<dyn AsyncInode>) -> Result<Arc<dyn AsyncInode>> {
        let inode = Arc::new(Dir::new(Self(RwLock::new(PidDirINode {
            process_ref: get_process(pid)?,
            this: Weak::default(),
            parent: Arc::clone(&parent),
            entries: HashMap::new(),
        }))));
        inode.inner().0.write().unwrap().this = Arc::downgrade(&inode);
        inode.inner().init_entries()?;
        Ok(inode)
    }

    fn init_entries(&self) -> Result<()> {
        let mut file = self.0.write().unwrap();
        // cmdline
        let cmdline_inode = ProcCmdlineINode::new(&file.process_ref);
        file.entries.insert(String::from("cmdline"), cmdline_inode);
        // cwd
        let cwd_inode = ProcCwdSymINode::new(&file.process_ref);
        file.entries.insert(String::from("cwd"), cwd_inode);
        // exe
        let exe_inode = ProcExeSymINode::new(&file.process_ref);
        file.entries.insert(String::from("exe"), exe_inode);
        // root
        let root_inode = ProcRootSymINode::new(&file.process_ref);
        file.entries.insert(String::from("root"), root_inode);
        // comm
        let comm_inode = ProcCommINode::new(&file.process_ref);
        file.entries.insert(String::from("comm"), comm_inode);
        // stat
        let stat_inode = ProcStatINode::new(&file.process_ref);
        file.entries.insert(String::from("stat"), stat_inode);
        // maps
        let maps_inode = ProcMapsINode::new(&file.process_ref);
        file.entries.insert(String::from("maps"), maps_inode);

        Ok(())
    }
}

#[async_trait]
impl DirProcINode for LockedPidDirINode {
    async fn find(&self, name: &str) -> Result<Arc<dyn AsyncInode>> {
        let file = self.0.read().unwrap();
        if name == "." {
            return Ok(file.this.upgrade().unwrap());
        }
        if name == ".." {
            return Ok(Arc::clone(&file.parent));
        }
        // The 'fd' entry holds 1 Arc of LockedPidDirINode, so the LockedPidDirINode
        // itself will hold 2 Arcs. This makes it cannot be dropped automatically.
        // We initialize the 'fd' here to avoid this.
        // TODO:: Try to find a better solution.
        if name == "fd" {
            let fd_inode =
                LockedProcFdDirINode::new(&file.process_ref, file.this.upgrade().unwrap());
            return Ok(fd_inode);
        }

        if let Some(inode) = file.entries.get(name) {
            Ok(Arc::clone(inode))
        } else {
            return_errno!(ENOENT, "");
        }
    }

    async fn iterate_entries(&self, mut ctx: &mut DirentWriterContext) -> Result<usize> {
        let file = self.0.read().unwrap();
        let idx = ctx.pos();

        // Write first two special entries
        write_first_two_entries!(idx, &mut ctx, &file);

        // Write the normal entries
        let skipped = if idx < 2 { 0 } else { idx - 2 };
        for (name, inode) in file.entries.iter().skip(skipped) {
            write_inode_entry!(&mut ctx, name, inode);
        }

        // Write the fd entry
        if idx <= 2 + file.entries.len() {
            write_entry!(&mut ctx, "fd", PROC_INO, FileType::Dir);
        }
        Ok(ctx.written_len())
    }
}
