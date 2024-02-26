use super::*;
use crate::util::kernel_alloc::KernelAlloc;
use crate::vm::USER_SPACE_VM_MANAGER;

pub struct MemInfoINode;

const KB: usize = 1024;

impl MemInfoINode {
    pub fn new() -> Arc<dyn INode> {
        Arc::new(File::new(Self))
    }
}

impl ProcINode for MemInfoINode {
    fn generate_data_in_bytes(&self) -> vfs::Result<Vec<u8>> {
        let total_ram = USER_SPACE_VM_MANAGER.get_total_size();
        let free_ram = USER_SPACE_VM_MANAGER.get_precise_free_size();
        let kernel_heap_total = KernelAlloc::get_kernel_heap_config();
        let kernel_heap_peak_used = KernelAlloc::get_kernel_heap_peak_used();
        let kernel_heap_in_use = if let Some(bytes) = KernelAlloc::get_kernel_mem_size() {
            format!("{} kB", bytes / KB)
        } else {
            "Feature not enabled".to_string()
        };
        Ok(format!(
            "MemTotal:              {} kB\n\
             MemFree:               {} kB\n\
             MemAvailable:          {} kB\n\
             KernelHeapTotal:       {} kB\n\
             KernelHeapPeakUsed:    {} kB\n\
             KernelHeapInUse:       {}\n",
            total_ram / KB,
            free_ram / KB,
            free_ram / KB,
            kernel_heap_total / KB,
            kernel_heap_peak_used / KB,
            kernel_heap_in_use,
        )
        .into_bytes())
    }
}
