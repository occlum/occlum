use xmas_elf::{ElfFile, program, P64};
use xmas_elf::sections;
use xmas_elf::symbol_table::{Entry64, DynEntry64};
use xmas_elf::program::{ProgramHeader};
use xmas_elf::sections::{Rela};
use xmas_elf::symbol_table::Entry;

pub fn print_program_headers(elf_file: &ElfFile) -> Result<(), &'static str> {
    println!("Program headers:");
    let ph_iter = elf_file.program_iter();
    for sect in ph_iter {
        program::sanity_check(sect, &elf_file)?;
        println!("\t{:?}", sect.get_type());
    }
    Ok(())
}

pub fn print_sections(elf_file: &ElfFile) -> Result<(), &'static str> {
    println!("Sections:");
    let mut sect_iter = elf_file.section_iter();
    sect_iter.next(); // Skip the first, dummy section
    for sect in sect_iter {
        sections::sanity_check(sect, &elf_file)?;
        println!("\t{}\n{:?}", sect.get_name(&elf_file)?, sect);
    }
    Ok(())
}

pub fn print_pltrel_section(elf_file: &ElfFile) -> Result<(), &'static str> {
    let rela_entries = get_pltrel_entries(elf_file)?;
    let dynsym_entries = get_dynsym_entries(elf_file)?;

    println!(".plt.rela section:");
    for entry in rela_entries {
        println!("\toffset: {}, symbol index: {}, type: {}, addend: {}",
                 entry.get_offset(),
                 entry.get_symbol_table_index(),
                 entry.get_type(),
                 entry.get_addend());

        let symidx = entry.get_symbol_table_index() as usize;
        let dynsym_entry = &dynsym_entries[symidx];
        println!("\t\t{} = {:?}",
                 dynsym_entry.get_name(&elf_file)?, dynsym_entry);
    }
    Ok(())
}

pub fn get_data_program_header<'b, 'a: 'b>(elf_file: &'b ElfFile<'a>)
    -> Result<ProgramHeader<'a>, &'static str>
{
    let mut ph_iter = elf_file.program_iter();
    ph_iter.find(|&ph| ph.get_type() == Ok(program::Type::Load) &&
                        !ph.flags().is_execute() &&
                        ph.flags().is_write() &&
                        ph.flags().is_read())
        .ok_or("Cannot find .data in the program header of ELF")
}

pub fn get_code_program_header<'b, 'a: 'b>(elf_file: &'b ElfFile<'a>)
    -> Result<ProgramHeader<'a>, &'static str>
{
    let mut ph_iter = elf_file.program_iter();
    ph_iter.find(|&ph| ph.get_type() == Ok(program::Type::Load) &&
                        ph.flags().is_execute() &&
                        !ph.flags().is_write() &&
                        ph.flags().is_read())
        .ok_or("Cannot find .text in the program header of ELF")
}

pub fn get_start_address<'b, 'a: 'b>(elf_file: &'b ElfFile<'a>)
    -> Result<usize, &'static str>
{
    let sym_entries = get_sym_entries(elf_file)?;

    for sym_entry in sym_entries {
        let sym_str = sym_entry.get_name(elf_file)?;
        if sym_str == "_start" {
            return Ok(sym_entry.value() as usize)
        }
    }

    Err("Cannot find _start symbol")
}

pub fn get_sym_entries<'b, 'a: 'b>(elf_file: &'b ElfFile<'a>)
    -> Result<&'a [Entry64], &'static str>
{
    elf_file.find_section_by_name(".symtab")
        .and_then(|symtab_section| {
            symtab_section.get_data(&elf_file).ok()
        }).and_then(|symbol_table| {
            match symbol_table {
                sections::SectionData::SymbolTable64(entries) => Some(entries),
                _ => None,
            }
        }).ok_or("Cannot find or load .dynsym section")
}

pub fn get_pltrel_entries<'b, 'a: 'b>(elf_file: &'b ElfFile<'a>)
    -> Result<&'a [Rela<P64>], &'static str>
{
    elf_file.find_section_by_name(".rela.plt")
        .and_then(|plt_rela_section| {
            plt_rela_section.get_data(&elf_file).ok()
        }).and_then(|rela_table| {
            match rela_table {
                sections::SectionData::Rela64(entries) => Some(entries),
                _ => None,
            }
        }).ok_or("Cannot find or load .rela.plt section")
}

pub fn get_dynsym_entries<'b, 'a: 'b>(elf_file: &'b ElfFile<'a>)
    -> Result<&'a [DynEntry64], &'static str>
{
    elf_file.find_section_by_name(".dynsym")
        .and_then(|dynamic_section| {
            dynamic_section.get_data(&elf_file).ok()
        }).and_then(|dynamic_table| {
            match dynamic_table {
                sections::SectionData::DynSymbolTable64(entries) => Some(entries),
                _ => None,
            }
        }).ok_or("Cannot find or load .dynsym section")
}
