use super::*;

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
