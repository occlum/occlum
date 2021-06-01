use super::*;
use crate::process::table::get_process;
use crate::process::ProcessRef;

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
        // fd
        let fd_inode = LockedProcFdDirINode::new(&file.process_ref, file.this.upgrade().unwrap());
        file.entries.insert(String::from("fd"), fd_inode);
        // root
        let root_inode = ProcRootSymINode::new(&file.process_ref);
        file.entries.insert(String::from("root"), root_inode);

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
                if let Some(s) = file.entries.keys().nth(i - 2) {
                    Ok(s.to_string())
                } else {
                    Err(FsError::EntryNotFound)
                }
            }
        }
    }
}

struct LockedProcFdDirINode(RwLock<ProcFdDirINode>);

struct ProcFdDirINode {
    process_ref: ProcessRef,
    this: Weak<Dir<LockedProcFdDirINode>>,
    parent: Arc<dyn INode>,
}

impl LockedProcFdDirINode {
    pub fn new(process_ref: &ProcessRef, parent: Arc<dyn INode>) -> Arc<dyn INode> {
        let inode = Arc::new(Dir::new(Self(RwLock::new(ProcFdDirINode {
            process_ref: Arc::clone(process_ref),
            this: Weak::default(),
            parent: Arc::clone(&parent),
        }))));
        inode.inner().0.write().unwrap().this = Arc::downgrade(&inode);
        inode
    }
}

impl DirProcINode for LockedProcFdDirINode {
    fn find(&self, name: &str) -> vfs::Result<Arc<dyn INode>> {
        let file = self.0.read().unwrap();
        if name == "." {
            return Ok(file.this.upgrade().unwrap());
        }
        if name == ".." {
            return Ok(Arc::clone(&file.parent));
        }
        let fd = name
            .parse::<FileDesc>()
            .map_err(|_| FsError::EntryNotFound)?;
        let fd_inode = FdSymINode::new(&file.process_ref, fd)?;
        Ok(fd_inode)
    }

    fn get_entry(&self, id: usize) -> vfs::Result<String> {
        match id {
            0 => Ok(String::from(".")),
            1 => Ok(String::from("..")),
            i => {
                // TODO: When to iterate the file table ?
                Err(FsError::EntryNotFound)
            }
        }
    }
}

pub struct ProcCmdlineINode(ProcessRef);

impl ProcCmdlineINode {
    pub fn new(process_ref: &ProcessRef) -> Arc<dyn INode> {
        Arc::new(File::new(Self(Arc::clone(process_ref))))
    }
}

impl ProcINode for ProcCmdlineINode {
    fn generate_data_in_bytes(&self) -> vfs::Result<Vec<u8>> {
        Ok(self.0.exec_path().to_owned().into_bytes())
    }
}

pub struct ProcExeSymINode(ProcessRef);

impl ProcExeSymINode {
    pub fn new(process_ref: &ProcessRef) -> Arc<dyn INode> {
        Arc::new(SymLink::new(Self(Arc::clone(process_ref))))
    }
}

impl ProcINode for ProcExeSymINode {
    fn generate_data_in_bytes(&self) -> vfs::Result<Vec<u8>> {
        Ok(self.0.exec_path().to_owned().into_bytes())
    }
}

pub struct ProcCwdSymINode(ProcessRef);

impl ProcCwdSymINode {
    pub fn new(process_ref: &ProcessRef) -> Arc<dyn INode> {
        Arc::new(SymLink::new(Self(Arc::clone(process_ref))))
    }
}

impl ProcINode for ProcCwdSymINode {
    fn generate_data_in_bytes(&self) -> vfs::Result<Vec<u8>> {
        let main_thread = self.0.main_thread().ok_or(FsError::EntryNotFound)?;
        let fs = main_thread.fs().read().unwrap();
        Ok(fs.cwd().to_owned().into_bytes())
    }
}

pub struct ProcRootSymINode(ProcessRef);

impl ProcRootSymINode {
    pub fn new(process_ref: &ProcessRef) -> Arc<dyn INode> {
        Arc::new(SymLink::new(Self(Arc::clone(process_ref))))
    }
}

impl ProcINode for ProcRootSymINode {
    fn generate_data_in_bytes(&self) -> vfs::Result<Vec<u8>> {
        let main_thread = self.0.main_thread().ok_or(FsError::EntryNotFound)?;
        let fs = main_thread.fs().read().unwrap();
        Ok(fs.root().to_owned().into_bytes())
    }
}

pub struct FdSymINode(FileRef);

impl FdSymINode {
    pub fn new(process_ref: &ProcessRef, fd: FileDesc) -> vfs::Result<Arc<dyn INode>> {
        let main_thread = process_ref.main_thread().ok_or(FsError::EntryNotFound)?;
        let file_ref = main_thread.file(fd).map_err(|_| FsError::EntryNotFound)?;
        Ok(Arc::new(SymLink::new(Self(Arc::clone(&file_ref)))))
    }
}

impl ProcINode for FdSymINode {
    fn generate_data_in_bytes(&self) -> vfs::Result<Vec<u8>> {
        let path = if let Ok(inode_file) = self.0.as_inode_file() {
            inode_file.abs_path().to_owned()
        } else {
            // TODO: Support other file types
            // For file descriptors for pipes and sockets,
            // the content is: type:[inode].
            // For file descriptors that have no corresponding inode,
            // the content is: anon_inode:[file-type]
            return Err(FsError::EntryNotFound);
        };
        Ok(path.into_bytes())
    }
}
