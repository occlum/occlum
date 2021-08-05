use super::*;

pub struct LockedProcFdDirINode(RwLock<ProcFdDirINode>);

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

struct FdSymINode(FileRef);

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
