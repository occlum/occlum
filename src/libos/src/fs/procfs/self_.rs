use super::*;

pub struct SelfSymINode;

impl SelfSymINode {
    pub fn new() -> Arc<dyn AsyncInode> {
        Arc::new(SymLink::new(Self))
    }
}

#[async_trait]
impl ProcINode for SelfSymINode {
    async fn generate_data_in_bytes(&self) -> Result<Vec<u8>> {
        let pid = current!().process().pid();
        Ok(pid.to_string().into_bytes())
    }
}
