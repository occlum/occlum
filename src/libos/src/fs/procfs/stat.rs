use super::*;
use crate::fs::hostfs::IntoFsError;
use std::untrusted::fs;

/// It returns most of the information from host OS's "/proc/stat",
/// and some fields will be filled in with LibOS's information.
pub struct StatINode;

impl StatINode {
    pub fn new() -> Arc<dyn AsyncInode> {
        Arc::new(File::new(Self))
    }
}

#[async_trait]
impl ProcINode for StatINode {
    async fn generate_data_in_bytes(&self) -> Result<Vec<u8>> {
        let mut host_stat = fs::read_to_string("/proc/stat")?;
        let boot_time = crate::time::up_time::boot_time_since_epoch()
            .as_secs()
            .to_string();
        fill_in_stat(&mut host_stat, "btime", &boot_time);
        let procs_running = {
            let mut processes = crate::process::table::get_all_processes();
            processes.retain(|p| p.status() == crate::process::ProcessStatus::Running);
            processes.len().to_string()
        };
        fill_in_stat(&mut host_stat, "procs_running", &procs_running);
        Ok(host_stat.into_bytes())
    }
}

fn fill_in_stat(stat: &mut String, pat: &str, val: &str) {
    let start_idx = stat.find(pat).unwrap_or_else(|| {
        error!("failed to find {} in host's /proc/stat", pat);
        panic!()
    }) + pat.len()
        + 1;
    let end_idx = stat
        .chars()
        .skip(start_idx)
        .position(|c| c == '\n')
        .unwrap_or_else(|| {
            error!("invalid format of host's /proc/stat");
            panic!()
        })
        + start_idx;
    stat.replace_range(start_idx..end_idx, val);
}
