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
        // maps
        let maps_inode = ProcMapsINode::new(&file.process_ref);
        file.entries.insert(String::from("maps"), maps_inode);

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

    fn iterate_entries(
        &self,
        offset: usize,
        visitor: &mut dyn DirentVisitor,
    ) -> vfs::Result<usize> {
        let file = self.0.read().unwrap();

        let try_iterate =
            |mut offset: &mut usize, mut visitor: &mut dyn DirentVisitor| -> vfs::Result<()> {
                // The two special entries
                visit_first_two_entries!(&mut visitor, &file, &mut offset);

                // The normal entries
                let start_offset = *offset;
                for (name, child) in file.entries.iter().skip(start_offset - 2) {
                    rcore_fs::visit_inode_entry!(&mut visitor, name, child, &mut offset);
                }

                // The fd entry
                if *offset == 2 + file.entries.len() {
                    rcore_fs::visit_entry!(
                        &mut visitor,
                        "fd",
                        PROC_INO as u64,
                        vfs::FileType::Dir,
                        &mut offset
                    );
                }

                Ok(())
            };

        let mut iterate_offset = offset;
        match try_iterate(&mut iterate_offset, visitor) {
            Err(e) if iterate_offset == offset => Err(e),
            _ => Ok(iterate_offset - offset),
        }
    }
}
