use super::*;

pub struct ProcExeSymINode(ProcessRef);

impl ProcExeSymINode {
    pub fn new(process_ref: &ProcessRef) -> Arc<dyn AsyncInode> {
        Arc::new(SymLink::new(Self(Arc::clone(process_ref))))
    }
}

#[async_trait]
impl ProcINode for ProcExeSymINode {
    async fn generate_data_in_bytes(&self) -> Result<Vec<u8>> {
        Ok(self.0.exec_path().to_owned().into_bytes())
    }
}
