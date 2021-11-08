use super::*;

pub struct ProcCmdlineINode(ProcessRef);

impl ProcCmdlineINode {
    pub fn new(process_ref: &ProcessRef) -> Arc<dyn INode> {
        Arc::new(File::new(Self(Arc::clone(process_ref))))
    }
}

impl ProcINode for ProcCmdlineINode {
    fn generate_data_in_bytes(&self) -> vfs::Result<Vec<u8>> {
        let cmdline = if let ProcessStatus::Zombie = self.0.status() {
            Vec::new()
        } else {
            // Null-terminated bytes
            std::ffi::CString::new(self.0.exec_path())
                .expect("failed to new CString")
                .into_bytes_with_nul()
        };
        Ok(cmdline)
    }
}
