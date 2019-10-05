use self::segment::*;
use super::*;
use std::ptr;
use xmas_elf::{header, program, sections, ElfFile};

pub fn do_init(
    elf_file: &ElfFile,
    elf_buf: &[u8],
    ldso_elf_file: &ElfFile,
    ldso_elf_buf: &[u8],
) -> Result<ProcessVM> {
    // Alloc all virtual memory areas
    let mut code_seg = get_code_segment(elf_file)?;
    let mut data_seg = get_data_segment(elf_file)?;
    let code_start = 0;
    let code_end = align_down(data_seg.get_mem_addr(), data_seg.get_mem_align());
    let data_start = code_end;
    let data_end = align_up(data_seg.get_mem_addr() + data_seg.get_mem_size(), 4096);
    let code_size = code_end - code_start;
    let data_size = data_end - data_start;

    let mut ldso_code_seg = get_code_segment(ldso_elf_file)?;
    let mut ldso_data_seg = get_data_segment(ldso_elf_file)?;
    let ldso_code_start = 0;
    let ldso_code_end = align_down(ldso_data_seg.get_mem_addr(), ldso_data_seg.get_mem_align());
    let ldso_data_start = ldso_code_end;
    let ldso_data_end = align_up(
        ldso_data_seg.get_mem_addr() + ldso_data_seg.get_mem_size(),
        4096,
    );
    let ldso_code_size = ldso_code_end - ldso_code_start;
    let ldso_data_size = ldso_data_end - ldso_data_start;

    let mut process_vm = ProcessVMBuilder::new(code_size, data_size)
        .ldso_code_size(ldso_code_size)
        .ldso_data_size(ldso_data_size)
        .build()
        .cause_err(|e| errno!(e.errno(), "failed to create process VM"))?;

    // Load code and data
    let process_base_addr = process_vm.get_code_range().start();
    code_seg.set_runtime_base(process_base_addr);
    data_seg.set_runtime_base(process_base_addr);
    code_seg.load_from_file(elf_buf);
    data_seg.load_from_file(elf_buf);

    // Load code and data of ld.so
    let ldso_base_addr = process_vm.get_ldso_code_range().start();
    ldso_code_seg.set_runtime_base(ldso_base_addr);
    ldso_data_seg.set_runtime_base(ldso_base_addr);
    ldso_code_seg.load_from_file(ldso_elf_buf);
    ldso_data_seg.load_from_file(ldso_elf_buf);

    // Relocate symbols
    //reloc_symbols(process_base_addr, elf_file)?;
    //link_syscalls(process_base_addr, elf_file)?;

    Ok(process_vm)
}

fn reloc_symbols(process_base_addr: usize, elf_file: &ElfFile) -> Result<()> {
    let rela_entries = elf_helper::get_rela_entries(elf_file, ".rela.dyn")?;
    for rela_entry in rela_entries {
        /*
        println!("\toffset: {:#X}, symbol index: {}, type: {}, addend: {:#X}",
             rela_entry.get_offset(),
             rela_entry.get_symbol_table_index(),
             rela_entry.get_type(),
             rela_entry.get_addend());
        */

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
/*
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
*/
extern "C" {
    fn __occlum_syscall(num: i32, arg0: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64) -> i64;
}
