use xmas_elf::symbol_table::Entry;
use xmas_elf::{header, program, sections};

use crate::prelude::*;

pub use xmas_elf::header::HeaderPt2 as ElfHeader;
pub use xmas_elf::program::{ProgramHeader, ProgramIter};

#[derive(Debug)]
pub struct ElfFile<'a> {
    elf_buf: &'a [u8],
    elf_inner: xmas_elf::ElfFile<'a>,
}

impl<'a> ElfFile<'a> {
    pub fn new(elf_buf: &'a [u8]) -> Result<ElfFile> {
        let elf_inner =
            xmas_elf::ElfFile::new(elf_buf).map_err(|e| errno!(ENOEXEC, "invalid ELF header"))?;
        Self::validate(&elf_inner)?;

        Ok(ElfFile { elf_buf, elf_inner })
    }

    pub fn program_headers<'b>(&'b self) -> ProgramIter<'b, 'a> {
        self.elf_inner.program_iter()
    }

    pub fn elf_header(&self) -> &ElfHeader {
        &self.elf_inner.header.pt2
    }

    pub fn as_slice(&self) -> &[u8] {
        self.elf_buf
    }

    fn validate(elf_inner: &xmas_elf::ElfFile) -> Result<()> {
        // Validate the ELF header
        xmas_elf::header::sanity_check(elf_inner)
            .map_err(|e| errno!(ENOEXEC, "invalid ELF header"))?;
        // Validate the segments
        for segment in elf_inner.program_iter() {
            segment.validate()?;
        }
        Ok(())
    }
}

pub trait ProgramHeaderExt {
    fn loadable(&self) -> bool;
    fn validate(&self) -> Result<()>;
}

impl<'a> ProgramHeaderExt for ProgramHeader<'a> {
    /// Is the segment loadable?
    fn loadable(&self) -> bool {
        let type_ = self.get_type().unwrap();
        type_ == xmas_elf::program::Type::Load
    }

    /// Do some basic sanity checks in case the ELF is corrupted somehow
    fn validate(&self) -> Result<()> {
        let ph64 = match self {
            ProgramHeader::Ph32(ph) => {
                return_errno!(ENOEXEC, "not support 32-bit ELF");
            }
            ProgramHeader::Ph64(ph64) => ph64,
        };
        if !ph64.align.is_power_of_two() {
            return_errno!(EINVAL, "invalid memory alignment");
        }
        if (ph64.offset % ph64.align) != (ph64.virtual_addr % ph64.align) {
            return_errno!(
                EINVAL,
                "memory address and file offset is not equal, per modulo"
            );
        }
        if ph64.mem_size < ph64.file_size {
            return_errno!(EINVAL, "memory size must be no less than file size");
        }
        Ok(())
    }
}
