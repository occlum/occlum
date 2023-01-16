use super::*;

pub struct ProcRootSymINode(ProcessRef);

impl ProcRootSymINode {
    pub fn new(process_ref: &ProcessRef) -> Arc<dyn AsyncInode> {
        Arc::new(SymLink::new(Self(Arc::clone(process_ref))))
    }
}

#[async_trait]
impl ProcINode for ProcRootSymINode {
    async fn generate_data_in_bytes(&self) -> Result<Vec<u8>> {
        let main_thread = self.0.main_thread().ok_or(errno!(ENOENT, ""))?;
        let fs = main_thread.fs();
        Ok(fs.root().abs_path().into_bytes())
    }

    fn is_volatile(&self) -> bool {
        true
    }
}
