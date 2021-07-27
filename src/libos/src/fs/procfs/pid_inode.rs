use super::*;
use crate::process::table::get_process;
use crate::process::{ProcessRef, ProcessStatus};

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
        // comm
        let comm_inode = ProcCommINode::new(&file.process_ref);
        file.entries.insert(String::from("comm"), comm_inode);
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
                let name = file
                    .entries
                    .keys()
                    .nth(i - 2)
                    .ok_or(FsError::EntryNotFound)?;
                Ok(name.to_owned())
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

        Ok(total_written_len)
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
                let file = self.0.read().unwrap();
                let main_thread = file
                    .process_ref
                    .main_thread()
                    .ok_or(FsError::EntryNotFound)?;
                let fds = main_thread.files().lock().unwrap().fds();
                let fd = fds.iter().nth(i - 2).ok_or(FsError::EntryNotFound)?;
                Ok(fd.to_string())
            }
        }
    }

    fn iterate_entries(&self, mut ctx: &mut DirentWriterContext) -> vfs::Result<usize> {
        let file = self.0.read().unwrap();
        let mut total_written_len = 0;
        let idx = ctx.pos();

        // Write first two special entries
        write_first_two_entries!(idx, &mut ctx, &file, &mut total_written_len);

        // Write the fd entries
        let skipped = if idx < 2 { 0 } else { idx - 2 };
        let main_thread = match file.process_ref.main_thread() {
            Some(main_thread) => main_thread,
            None => {
                return Ok(total_written_len);
            }
        };
        let fds = main_thread.files().lock().unwrap().fds();
        for fd in fds.iter().skip(skipped) {
            write_entry!(
                &mut ctx,
                &fd.to_string(),
                PROC_INO,
                vfs::FileType::SymLink,
                &mut total_written_len
            );
        }

        Ok(total_written_len)
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
        let cmdline = if let ProcessStatus::Zombie = self.0.status() {
            Vec::new()
        } else {
            // Null-terminated bytes
            std::ffi::CString::new(self.0.exec_path())
                .expect("failed to new CString")
                .into_bytes_with_nul()
        };
        Ok(cmdline)
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

pub struct ProcCommINode(ProcessRef);

impl ProcCommINode {
    pub fn new(process_ref: &ProcessRef) -> Arc<dyn INode> {
        Arc::new(File::new(Self(Arc::clone(process_ref))))
    }
}

impl ProcINode for ProcCommINode {
    fn generate_data_in_bytes(&self) -> vfs::Result<Vec<u8>> {
        let main_thread = self.0.main_thread().ok_or(FsError::EntryNotFound)?;
        let mut comm = main_thread.name().as_c_str().to_bytes().to_vec();
        // Add '\n' at the end to make the result same with Linux
        comm.push(b'\n');
        Ok(comm)
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
