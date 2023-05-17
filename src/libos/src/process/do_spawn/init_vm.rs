use std::ptr;

use super::super::elf_file::ElfFile;
use crate::misc::{resource_t, rlimit_t};
use crate::prelude::*;
use crate::vm::{ProcessVM, ProcessVMBuilder};

pub async fn do_init<'a, 'b>(
    elf_file: &'b ElfFile<'a>,
    ldso_elf_file: &'b ElfFile<'a>,
) -> Result<ProcessVM> {
    let mut process_vm = if current!().process().pid() == 0 {
        // Parent process is idle process and we can skip checking rlimit because main
        // process will directly use memory configuration in Occlum.json
        ProcessVMBuilder::new(vec![elf_file, ldso_elf_file])
            .build()
            .await
            .cause_err(|e| errno!(e.errno(), "failed to create process VM"))?
    } else {
        // Parent process is not idle process. Inherit parent process's resource limit.
        let rlimit = current!().rlimits().lock().unwrap().clone();
        let child_heap_size = rlimit.get(resource_t::RLIMIT_DATA).get_cur();
        let child_stack_size = rlimit.get(resource_t::RLIMIT_STACK).get_cur();

        debug!(
            "new process: heap_size = {:?}, stack_size = {:?}",
            child_heap_size, child_stack_size
        );

        ProcessVMBuilder::new(vec![elf_file, ldso_elf_file])
            .set_heap_size(child_heap_size as usize)
            .set_stack_size(child_stack_size as usize)
            .clone()
            .build()
            .await
            .cause_err(|e| errno!(e.errno(), "failed to create process VM"))?
    };

    // Relocate symbols
    //reloc_symbols(process_base_addr, elf_file)?;
    //link_syscalls(process_base_addr, elf_file)?;
    Ok(process_vm)
}

/*
fn reloc_symbols(process_base_addr: usize, elf_file: &ElfFile) -> Result<()> {
    let rela_entries = elf_helper::get_rela_entries(elf_file, ".rela.dyn")?;
    for rela_entry in rela_entries {
        trace!(
            "\toffset: {:#X}, symbol index: {}, type: {}, addend: {:#X}",
            rela_entry.get_offset(),
            rela_entry.get_symbol_table_index(),
            rela_entry.get_type(),
            rela_entry.get_addend()
        );

        match rela_entry.get_type() {
            // reloc type == R_X86_64_RELATIVE
            8 if rela_entry.get_symbol_table_index() == 0 => {
                let rela_addr = process_base_addr + rela_entry.get_offset() as usize;
                let rela_val = process_base_addr + rela_entry.get_addend() as usize;
                unsafe {
                    ptr::write_unaligned(rela_addr as *mut usize, rela_val);
                }
            }
            // TODO: need to handle other relocation types
            _ => {}
        }
    }
    Ok(())
}

fn link_syscalls(process_base_addr: usize, elf_file: &ElfFile) -> Result<()> {
    let syscall_addr = __occlum_syscall as *const () as usize;

    let rela_entries = elf_helper::get_rela_entries(elf_file, ".rela.plt")?;
    let dynsym_entries = elf_helper::get_dynsym_entries(elf_file)?;
    for rela_entry in rela_entries {
        let dynsym_idx = rela_entry.get_symbol_table_index() as usize;
        let dynsym_entry = &dynsym_entries[dynsym_idx];
        let dynsym_str = dynsym_entry
            .get_name(elf_file)
            .map_err(|e| Error::new(Errno::ENOEXEC, "Failed to get the name of dynamic symbol"))?;

        if dynsym_str == "__occlum_syscall" {
            let rela_addr = process_base_addr + rela_entry.get_offset() as usize;
            unsafe {
                ptr::write_unaligned(rela_addr as *mut usize, syscall_addr);
            }
        }
    }

    Ok(())
}

extern "C" {
    fn __occlum_syscall(num: i32, arg0: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64) -> i64;
}
*/
