use super::*;

use crate::vm::{ChunkType, VMArea, VMPerms, VMRange};

// This file is to implement /proc/self(pid)/maps file system.
//
// Print format:
// vmrange_start-vmrange_end, permission, shared/private, offset, device ID, inode, pathname
//
// Example:
// - cat /proc/self/maps
// 555555554000-555555556000 r--p 00000000 08:12 39321752                   /usr/bin/cat
// 555555556000-55555555b000 r-xp 00002000 08:12 39321752                   /usr/bin/cat
// 55555555b000-55555555e000 r--p 00007000 08:12 39321752                   /usr/bin/cat
// 55555555e000-55555555f000 r--p 00009000 08:12 39321752                   /usr/bin/cat
// 55555555f000-555555560000 rw-p 0000a000 08:12 39321752                   /usr/bin/cat
// 555555560000-555555581000 rw-p 00000000 00:00 0                          [heap]
// 7ffff7536000-7ffff7558000 rw-p 00000000 00:00 0
// 7ffff7558000-7ffff7dc8000 r--p 00000000 08:12 39322175                   /usr/lib/locale/locale-archive
// 7ffff7dc8000-7ffff7dea000 r--p 00000000 08:12 39324754                   /usr/lib/x86_64-linux-gnu/libc-2.31.so
// 7ffff7dea000-7ffff7f62000 r-xp 00022000 08:12 39324754                   /usr/lib/x86_64-linux-gnu/libc-2.31.so
// 7ffff7f62000-7ffff7fb0000 r--p 0019a000 08:12 39324754                   /usr/lib/x86_64-linux-gnu/libc-2.31.so
// 7ffff7fb0000-7ffff7fb4000 r--p 001e7000 08:12 39324754                   /usr/lib/x86_64-linux-gnu/libc-2.31.so
// 7ffff7fb4000-7ffff7fb6000 rw-p 001eb000 08:12 39324754                   /usr/lib/x86_64-linux-gnu/libc-2.31.so
// 7ffff7fb6000-7ffff7fbc000 rw-p 00000000 00:00 0
// 7ffff7fcf000-7ffff7fd0000 r--p 00000000 08:12 39324750                   /usr/lib/x86_64-linux-gnu/ld-2.31.so
// 7ffff7fd0000-7ffff7ff3000 r-xp 00001000 08:12 39324750                   /usr/lib/x86_64-linux-gnu/ld-2.31.so
// 7ffff7ff3000-7ffff7ffb000 r--p 00024000 08:12 39324750                   /usr/lib/x86_64-linux-gnu/ld-2.31.so
// 7ffff7ffc000-7ffff7ffd000 r--p 0002c000 08:12 39324750                   /usr/lib/x86_64-linux-gnu/ld-2.31.so
// 7ffff7ffd000-7ffff7ffe000 rw-p 0002d000 08:12 39324750                   /usr/lib/x86_64-linux-gnu/ld-2.31.so
// 7ffff7ffe000-7ffff7fff000 rw-p 00000000 00:00 0
// 7ffffffde000-7ffffffff000 rw-p 00000000 00:00 0                          [stack]
// 80000006b000-80000006f000 r--p 00000000 00:00 0                          [vvar]
// 80000006f000-800000071000 r-xp 00000000 00:00 0                          [vdso]
// ffffffffff600000-ffffffffff601000 --xp 00000000 00:00 0                  [vsyscall]
//
// Known limitation:
// - Device ID is not provided by FS
// - Not shown in address order

pub struct ProcMapsINode(ProcessRef);

impl ProcMapsINode {
    pub fn new(process_ref: &ProcessRef) -> Arc<dyn AsyncInode> {
        Arc::new(File::new(Self(Arc::clone(process_ref))))
    }
}

#[async_trait]
impl ProcINode for ProcMapsINode {
    async fn generate_data_in_bytes(&self) -> Result<Vec<u8>> {
        let result_string = {
            let main_thread = self.0.main_thread().unwrap();
            let process_vm = main_thread.vm();
            let heap_range = process_vm.heap_range();
            let stack_range = process_vm.stack_range();

            let process_vm_chunks = process_vm.mem_chunks().read().await;
            let mut vmas_info = String::new();
            for chunk in process_vm_chunks.iter() {
                vmas_info.extend_one(match chunk.internal() {
                    ChunkType::SingleVMA(vma) => {
                        let range = chunk.range();
                        let heap_or_stack = if range == heap_range {
                            Some(" [heap]")
                        } else if range == stack_range {
                            Some(" [stack]")
                        } else {
                            None
                        };
                        let vma = vma.lock().await;
                        get_output_for_vma(&vma, heap_or_stack).await
                    }
                    ChunkType::MultiVMA(internal_manager) => {
                        let internal = internal_manager.lock().await;
                        let vmas_list = internal
                            .chunk_manager()
                            .vmas()
                            .iter()
                            .map(|vma_obj| vma_obj.vma().clone())
                            .collect::<Vec<_>>();
                        let mut vma_info = String::new();
                        for vma in vmas_list {
                            vma_info += &get_output_for_vma(&vma, None).await;
                        }
                        vma_info
                    }
                });
            }
            vmas_info
        };

        Ok(result_string.into_bytes())
    }
}

async fn get_output_for_vma(vma: &VMArea, heap_or_stack: Option<&str>) -> String {
    let range = vma.range();
    let perms = vma.perms();

    // Skip sentry vmas
    if range.size() == 0 {
        return String::new();
    }

    let (file_path, offset, device_id, inode_num) = {
        if let Some((file, offset)) = vma.init_file() {
            let file_handle = file.as_async_file_handle().unwrap();
            let file_path = file_handle.dentry().abs_path();
            let inode = file_handle.dentry().inode();
            let inode_metadata = inode.metadata().await.unwrap();
            let inode_num = inode_metadata.inode;
            let device_id = inode_metadata.dev;
            (file_path, offset, device_id, inode_num)
        } else if heap_or_stack.is_some() {
            (heap_or_stack.unwrap(), 0, 0, 0)
        } else {
            ("", 0, 0, 0)
        }
    };

    let shared = vma.writeback_file().is_some();
    print_each_map(
        range, perms, shared, offset, device_id, inode_num, file_path,
    )
}

fn print_each_map(
    range: &VMRange,
    perms: VMPerms,
    shared: bool,
    offset: usize,
    device_id: usize,
    inode_num: usize,
    file_path: &str,
) -> String {
    let result_str = format!(
        "{:x}-{:x} {}{} {:08x} {} {}      {}\n",
        range.start(),
        range.end(),
        perms.display(),
        if shared { "s" } else { "p" },
        offset,
        device_id,
        inode_num,
        file_path
    );
    result_str
}
