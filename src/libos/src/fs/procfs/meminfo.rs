use super::*;
use crate::vm::USER_SPACE_VM_MANAGER;

pub struct MemInfoINode;

const KB: usize = 1024;

impl MemInfoINode {
    pub fn new() -> Arc<dyn AsyncInode> {
        Arc::new(File::new(Self))
    }
}

#[async_trait]
impl ProcINode for MemInfoINode {
    async fn generate_data_in_bytes(&self) -> Result<Vec<u8>> {
        let total_ram = USER_SPACE_VM_MANAGER.get_total_size();
        let free_ram = current!().vm().get_free_size().await;
        Ok(format!(
            "MemTotal:       {} kB\n\
             MemFree:        {} kB\n\
             MemAvailable:   {} kB\n",
            total_ram / KB,
            free_ram / KB,
            free_ram / KB,
        )
        .into_bytes())
    }
}
