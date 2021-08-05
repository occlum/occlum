use super::*;
use crate::process::do_getpid;

pub struct SelfSymINode;

impl SelfSymINode {
    pub fn new() -> Arc<dyn INode> {
        Arc::new(SymLink::new(Self))
    }
}

impl ProcINode for SelfSymINode {
    fn generate_data_in_bytes(&self) -> vfs::Result<Vec<u8>> {
        Ok(do_getpid().unwrap().to_string().into_bytes())
    }
}
