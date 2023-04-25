use super::*;

pub struct LockedProcFdDirINode(RwLock<ProcFdDirINode>);

struct ProcFdDirINode {
    process_ref: ProcessRef,
    this: Weak<Dir<LockedProcFdDirINode>>,
    parent: Arc<dyn AsyncInode>,
}

impl LockedProcFdDirINode {
    pub fn new(process_ref: &ProcessRef, parent: Arc<dyn AsyncInode>) -> Arc<dyn AsyncInode> {
        let inode = Arc::new(Dir::new(Self(RwLock::new(ProcFdDirINode {
            process_ref: Arc::clone(process_ref),
            this: Weak::default(),
            parent: Arc::clone(&parent),
        }))));
        inode.inner().0.write().unwrap().this = Arc::downgrade(&inode);
        inode
    }
}

#[async_trait]
impl DirProcINode for LockedProcFdDirINode {
    async fn find(&self, name: &str) -> Result<Arc<dyn AsyncInode>> {
        let file = self.0.read().unwrap();
        if name == "." {
            return Ok(file.this.upgrade().unwrap());
        }
        if name == ".." {
            return Ok(Arc::clone(&file.parent));
        }
        let fd = name.parse::<FileDesc>()?;
        let fd_inode = FdSymINode::new(&file.process_ref, fd)?;
        Ok(fd_inode)
    }

    async fn iterate_entries(&self, mut ctx: &mut DirentWriterContext) -> Result<usize> {
        let file = self.0.read().unwrap();
        let idx = ctx.pos();

        // Write first two special entries
        write_first_two_entries!(idx, &mut ctx, &file);

        // Write the fd entries
        let skipped = if idx < 2 { 0 } else { idx - 2 };
        let main_thread = match file.process_ref.main_thread() {
            Some(main_thread) => main_thread,
            None => {
                return Ok(ctx.written_len());
            }
        };
        let fds = main_thread.files().lock().unwrap().fds();
        for fd in fds.iter().skip(skipped) {
            write_entry!(&mut ctx, &fd.to_string(), PROC_INO, FileType::SymLink);
        }
        Ok(ctx.written_len())
    }
}

struct FdSymINode(FileRef);

impl FdSymINode {
    pub fn new(process_ref: &ProcessRef, fd: FileDesc) -> Result<Arc<dyn AsyncInode>> {
        let main_thread = process_ref.main_thread().ok_or(errno!(ENOENT, ""))?;
        let file_ref = main_thread.file(fd)?;
        Ok(Arc::new(SymLink::new(Self(file_ref.clone()))))
    }
}

#[async_trait]
impl ProcINode for FdSymINode {
    async fn generate_data_in_bytes(&self) -> Result<Vec<u8>> {
        let path = if let Some(async_file_handle) = self.0.as_async_file_handle() {
            async_file_handle.dentry().abs_path().to_owned()
        } else {
            // TODO: Support other file types
            // For file descriptors for pipes and sockets,
            // the content is: type:[inode].
            // For file descriptors that have no corresponding inode,
            // the content is: anon_inode:[file-type]
            return_errno!(ENOENT, "");
        };
        Ok(path.into_bytes())
    }
}
