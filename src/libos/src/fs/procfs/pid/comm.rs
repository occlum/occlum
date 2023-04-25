use super::*;

pub struct ProcCommINode(ProcessRef);

impl ProcCommINode {
    pub fn new(process_ref: &ProcessRef) -> Arc<dyn AsyncInode> {
        Arc::new(File::new(Self(Arc::clone(process_ref))))
    }
}

#[async_trait]
impl ProcINode for ProcCommINode {
    async fn generate_data_in_bytes(&self) -> Result<Vec<u8>> {
        let main_thread = self.0.main_thread().ok_or(errno!(ENOENT, ""))?;
        let mut comm = main_thread.name().as_c_str().to_bytes().to_vec();
        // Add '\n' at the end to make the result same with Linux
        comm.push(b'\n');
        Ok(comm)
    }
}
