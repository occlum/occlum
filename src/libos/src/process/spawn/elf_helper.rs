use super::*;

use xmas_elf::program::ProgramHeader;
use xmas_elf::sections::Rela;
use xmas_elf::symbol_table::Entry;
use xmas_elf::symbol_table::{DynEntry64, Entry64};
use xmas_elf::{program, sections, ElfFile, P64};

#[derive(Clone, Default, Copy, Debug)]
pub struct ProgramHeaderInfo {
    pub addr: usize,
    pub entry_size: usize,
    pub entry_num: usize,
}

pub fn get_program_header_info(elf_file: &ElfFile) -> Result<ProgramHeaderInfo> {
    let elf_header = &elf_file.header.pt2;
    Ok(ProgramHeaderInfo {
        addr: elf_header.ph_offset() as usize,
        entry_size: elf_header.ph_entry_size() as usize,
        entry_num: elf_header.ph_count() as usize,
    })
}

pub fn print_program_headers(elf_file: &ElfFile) -> Result<()> {
    println!("Program headers:");
    let ph_iter = elf_file.program_iter();
    for sect in ph_iter {
        program::sanity_check(sect, &elf_file)
            .map_err(|e| errno!(ENOEXEC, "sanity check for program header failed"))?;
        println!("\t{:?}", sect.get_type());
    }
    Ok(())
}

pub fn print_sections(elf_file: &ElfFile) -> Result<()> {
    println!("Sections:");
    let mut sect_iter = elf_file.section_iter();
    sect_iter.next(); // Skip the first, dummy section
    for sect in sect_iter {
        sections::sanity_check(sect, &elf_file)
            .map_err(|e| errno!(ENOEXEC, "sanity check for program header failed"))?;
        let sec_name = sect
            .get_name(&elf_file)
            .map_err(|e| errno!(ENOEXEC, "failed to get section name"))?;
        println!("\t{}\n{:?}", sec_name, sect);
    }
    Ok(())
}

pub fn print_rela_plt_section(elf_file: &ElfFile) -> Result<()> {
    let rela_entries = get_rela_entries(elf_file, ".rela.plt")
        .map_err(|e| errno!(ENOEXEC, "failed to get .pltrel entries"))?;
    let dynsym_entries = get_dynsym_entries(elf_file)
        .map_err(|e| errno!(ENOEXEC, "failed to get .dynsym entries"))?;

    println!(".rela.plt section:");
    for entry in rela_entries {
        println!(
            "\toffset: {}, symbol index: {}, type: {}, addend: {}",
            entry.get_offset(),
            entry.get_symbol_table_index(),
            entry.get_type(),
            entry.get_addend()
        );

        let symidx = entry.get_symbol_table_index() as usize;
        let dynsym_entry = &dynsym_entries[symidx];
        let dynsym_name = dynsym_entry
            .get_name(&elf_file)
            .map_err(|e| errno!(ENOEXEC, "failed to get the name of a dynamic symbol"))?;
        println!("\t\t{} = {:?}", dynsym_name, dynsym_entry);
    }
    Ok(())
}

pub fn get_data_program_header<'b, 'a: 'b>(elf_file: &'b ElfFile<'a>) -> Result<ProgramHeader<'a>> {
    let mut ph_iter = elf_file.program_iter();
    ph_iter
        .find(|&ph| {
            ph.get_type() == Ok(program::Type::Load)
                && !ph.flags().is_execute()
                && ph.flags().is_write()
                && ph.flags().is_read()
        })
        .ok_or_else(|| errno!(ENOEXEC, "failed to get the data segment"))
}

pub fn get_code_program_header<'b, 'a: 'b>(elf_file: &'b ElfFile<'a>) -> Result<ProgramHeader<'a>> {
    let mut ph_iter = elf_file.program_iter();
    ph_iter
        .find(|&ph| {
            ph.get_type() == Ok(program::Type::Load)
                && ph.flags().is_execute()
                && !ph.flags().is_write()
                && ph.flags().is_read()
        })
        .ok_or_else(|| errno!(ENOEXEC, "failed to get the code segment"))
}

pub fn get_start_address<'b, 'a: 'b>(elf_file: &'b ElfFile<'a>) -> Result<usize> {
    let elf_header = &elf_file.header.pt2;
    Ok(elf_header.entry_point() as usize)
}

pub fn get_sym_entries<'b, 'a: 'b>(elf_file: &'b ElfFile<'a>) -> Result<&'a [Entry64]> {
    elf_file
        .find_section_by_name(".symtab")
        .and_then(|symtab_section| symtab_section.get_data(&elf_file).ok())
        .and_then(|symbol_table| match symbol_table {
            sections::SectionData::SymbolTable64(entries) => Some(entries),
            _ => None,
        })
        .ok_or_else(|| errno!(ENOEXEC, "failed get the symbol entries"))
}

pub fn get_rela_entries<'b, 'a: 'b>(
    elf_file: &'b ElfFile<'a>,
    sec_name: &'b str,
) -> Result<&'a [Rela<P64>]> {
    elf_file
        .find_section_by_name(sec_name)
        .and_then(|plt_rela_section| plt_rela_section.get_data(&elf_file).ok())
        .and_then(|rela_table| match rela_table {
            sections::SectionData::Rela64(entries) => Some(entries),
            _ => None,
        })
        .ok_or_else(|| errno!(ENOEXEC, "failed to get .rela.plt entries"))
}

pub fn get_dynsym_entries<'b, 'a: 'b>(elf_file: &'b ElfFile<'a>) -> Result<&'a [DynEntry64]> {
    elf_file
        .find_section_by_name(".dynsym")
        .and_then(|dynamic_section| dynamic_section.get_data(&elf_file).ok())
        .and_then(|dynamic_table| match dynamic_table {
            sections::SectionData::DynSymbolTable64(entries) => Some(entries),
            _ => None,
        })
        .ok_or_else(|| errno!(ENOEXEC, "failed to get .dynsym entries"))
}
