use super::*;
use std::untrusted::fs;

pub struct CpuInfoINode;

impl CpuInfoINode {
    pub fn new() -> Arc<dyn AsyncInode> {
        Arc::new(File::new(Self))
    }
}

#[async_trait]
impl ProcINode for CpuInfoINode {
    async fn generate_data_in_bytes(&self) -> Result<Vec<u8>> {
        Ok(CPUINFO.to_vec())
    }
}

lazy_static! {
    static ref CPUINFO: Vec<u8> = get_untrusted_cpuinfo().unwrap();
}

fn get_untrusted_cpuinfo() -> Result<Vec<u8>> {
    let cpuinfo = fs::read_to_string("/proc/cpuinfo")?.into_bytes();
    // TODO: do sanity check ?
    Ok(cpuinfo)
}
