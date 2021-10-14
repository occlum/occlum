use super::*;
use crate::process::table::get_process;
use crate::process::{ProcessRef, ProcessStatus};

use self::cmdline::ProcCmdlineINode;
use self::comm::ProcCommINode;
use self::cwd::ProcCwdSymINode;
use self::exe::ProcExeSymINode;
use self::fd::LockedProcFdDirINode;
use self::root::ProcRootSymINode;
use self::stat::ProcStatINode;

mod cmdline;
mod comm;
mod cwd;
mod exe;
mod fd;
mod root;
mod stat;

pub struct LockedPidDirINode(RwLock<PidDirINode>);

struct PidDirINode {
    process_ref: ProcessRef,
    this: Weak<Dir<LockedPidDirINode>>,
    parent: Arc<dyn INode>,
    entries: HashMap<String, Arc<dyn INode>>,
}

impl LockedPidDirINode {
    pub fn new(pid: pid_t, parent: Arc<dyn INode>) -> vfs::Result<Arc<dyn INode>> {
        let inode = Arc::new(Dir::new(Self(RwLock::new(PidDirINode {
            process_ref: get_process(pid).map_err(|_| FsError::EntryNotFound)?,
            this: Weak::default(),
            parent: Arc::clone(&parent),
            entries: HashMap::new(),
        }))));
        inode.inner().0.write().unwrap().this = Arc::downgrade(&inode);
        inode.inner().init_entries()?;
        Ok(inode)
    }

    fn init_entries(&self) -> vfs::Result<()> {
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

        Ok(())
    }
}

impl DirProcINode for LockedPidDirINode {
    fn find(&self, name: &str) -> vfs::Result<Arc<dyn INode>> {
        let file = self.0.read().unwrap();
        if name == "." {
            return Ok(file.this.upgrade().unwrap());
        }
        if name == ".." {
            return Ok(Arc::clone(&file.parent));
        }
        // The 'fd' entry holds 1 Arc of LockedPidDirINode, so the LockedPidDirINode
        // ifself will hold 2 Arcs. This makes it cannot be dropped automatically.
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
            Err(FsError::EntryNotFound)
        }
    }

    fn get_entry(&self, id: usize) -> vfs::Result<String> {
        match id {
            0 => Ok(String::from(".")),
            1 => Ok(String::from("..")),
            i => {
                let file = self.0.read().unwrap();
                if let Some(name) = file.entries.keys().nth(i - 2) {
                    Ok(name.to_owned())
                } else if i == file.entries.len() + 2 {
                    Ok(String::from("fd"))
                } else {
                    Err(FsError::EntryNotFound)
                }
            }
        }
    }

    fn iterate_entries(&self, mut ctx: &mut DirentWriterContext) -> vfs::Result<usize> {
        let file = self.0.read().unwrap();
        let mut total_written_len = 0;
        let idx = ctx.pos();

        // Write first two special entries
        write_first_two_entries!(idx, &mut ctx, &file, &mut total_written_len);

        // Write the normal entries
        let skipped = if idx < 2 { 0 } else { idx - 2 };
        for (name, inode) in file.entries.iter().skip(skipped) {
            write_inode_entry!(&mut ctx, name, inode, &mut total_written_len);
        }

        // Write the fd entry
        if idx <= 2 + file.entries.len() {
            write_entry!(
                &mut ctx,
                "fd",
                PROC_INO,
                vfs::FileType::Dir,
                &mut total_written_len
            );
        }

        Ok(total_written_len)
    }
}
