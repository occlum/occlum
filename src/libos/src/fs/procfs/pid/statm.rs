use super::*;

use crate::vm::{ChunkType, VMArea, VMPerms, VMRange, PAGE_SIZE};

// This file is to implement /proc/self(pid)/statm file system.

pub struct ProcStatmINode(ProcessRef);

impl ProcStatmINode {
    pub fn new(process_ref: &ProcessRef) -> Arc<dyn INode> {
        Arc::new(File::new(Self(Arc::clone(process_ref))))
    }
}

impl ProcINode for ProcStatmINode {
    fn generate_data_in_bytes(&self) -> vfs::Result<Vec<u8>> {
        let result_string = {
            let main_thread = self.0.main_thread().unwrap();
            let process_vm = main_thread.vm();
            let heap_range = process_vm.heap_range();
            let stack_range = process_vm.stack_range();

            // Measured in pages
            let virtual_mem_usage = process_vm.get_in_use_size() / PAGE_SIZE;

            // We are unable to get the resident memory size in the enclave.
            // Just consider the same as virtual mem usage.
            let resident_mem = virtual_mem_usage;

            let data = (heap_range.size() + stack_range.size()) / PAGE_SIZE;

            // Dummy
            let shared_resident_mem = 0;
            let text = 0;

            // Always 0
            let lib = 0;
            let dirty_pages = 0;

            print_statm(
                virtual_mem_usage,
                resident_mem,
                shared_resident_mem,
                text,
                lib,
                data,
                dirty_pages,
            )
        };

        Ok(result_string.into_bytes())
    }
}

fn print_statm(
    size: usize,
    resident: usize,
    shared: usize,
    text: usize,
    lib: usize,
    data: usize,
    dt: usize,
) -> String {
    let result_str = format!(
        "{} {} {} {} {} {} {}\n",
        size, resident, shared, text, lib, data, dt
    );
    result_str
}
