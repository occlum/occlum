use super::*;

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
