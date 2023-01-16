use super::*;

pub struct ProcCwdSymINode(ProcessRef);

impl ProcCwdSymINode {
    pub fn new(process_ref: &ProcessRef) -> Arc<dyn AsyncInode> {
        Arc::new(SymLink::new(Self(Arc::clone(process_ref))))
    }
}

#[async_trait]
impl ProcINode for ProcCwdSymINode {
    async fn generate_data_in_bytes(&self) -> Result<Vec<u8>> {
        let main_thread = self.0.main_thread().ok_or(errno!(ENOENT, ""))?;
        let fs = main_thread.fs();
        Ok(fs.cwd().abs_path().into_bytes())
    }

    fn is_volatile(&self) -> bool {
        true
    }
}
