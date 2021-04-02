use crate::prelude::*;
use std::ffi::CStr;
use std::fmt;
use std::str;

use goblin::container::{Container, Ctx};
pub use goblin::elf::header::Header as ElfHeader;
use goblin::elf::{program_header, Elf, ProgramHeader};
use goblin::elf64::header::ET_DYN;
use rcore_fs::vfs::INode;
use scroll::{self, ctx, Pread};

const ELF64_HDR_SIZE: usize = 64;

pub struct ElfFile<'a> {
    elf_buf: &'a [u8],
    elf_inner: Elf<'a>,
    file_inode: &'a Arc<dyn INode>,
}

impl<'a> Debug for ElfFile<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ElfFile {{ inode: ???, elf_buf: {:?}, elf_inner: {:?} }}",
            self.elf_buf, self.elf_inner,
        )
    }
}

impl<'a> ElfFile<'a> {
    pub fn new(
        file_inode: &'a Arc<dyn INode>,
        mut elf_buf: &'a mut [u8],
        header: ElfHeader,
    ) -> Result<ElfFile<'a>> {
        let ctx = Ctx {
            le: scroll::Endian::Little,
            container: Container::Big,
        };

        // Get a dummy Elf with only header. Fill needed parts later.
        let mut elf_inner = goblin::elf::Elf::lazy_parse(header)
            .map_err(|e| errno!(ENOEXEC, "invalid ELF format"))?;

        let program_headers = ProgramHeader::parse(
            elf_buf,
            header.e_phoff as usize,
            header.e_phnum as usize,
            ctx,
        )
        .map_err(|e| errno!(ENOEXEC, "invalid program headers"))?;

        // read interpreter path
        let mut intepreter_count = 0;
        let mut intepreter_offset = 0;
        for ph in &program_headers {
            ph.validate()?;
            if ph.p_type == program_header::PT_INTERP && ph.p_filesz != 0 {
                intepreter_count = ph.p_filesz as usize;
                intepreter_offset = ph.p_offset as usize;
                trace!(
                    "PT_INTERP offset = {}, count = {}",
                    intepreter_offset,
                    intepreter_count
                );
                file_inode.read_at(
                    intepreter_offset,
                    &mut elf_buf[intepreter_offset..intepreter_offset + intepreter_count],
                );
                break;
            }
        }

        let interpreter = if intepreter_count == 0 {
            None
        } else {
            let cstr: &CStr = CStr::from_bytes_with_nul(
                &elf_buf[intepreter_offset..intepreter_offset + intepreter_count],
            )
            .map_err(|e| errno!(ENOEXEC, "invalid interpreter path"))?;
            cstr.to_str().ok()
        };
        trace!("interpreter = {:?}", interpreter);
        elf_inner.program_headers = program_headers;
        elf_inner.interpreter = interpreter;
        Ok(ElfFile {
            elf_buf,
            elf_inner,
            file_inode,
        })
    }

    pub fn program_headers<'b>(&'b self) -> impl Iterator<Item = &'b ProgramHeader> {
        self.elf_inner.program_headers.iter()
    }

    pub fn elf_header(&self) -> &ElfHeader {
        &self.elf_inner.header
    }

    pub fn elf_interpreter(&self) -> Option<&'a str> {
        self.elf_inner.interpreter
    }

    pub fn as_slice(&self) -> &[u8] {
        self.elf_buf
    }

    pub fn file_inode(&self) -> &Arc<dyn INode> {
        self.file_inode
    }

    pub fn parse_elf_hdr(inode: &Arc<dyn INode>, elf_buf: &mut Vec<u8>) -> Result<ElfHeader> {
        // TODO: Sanity check the number of program headers..
        let mut phdr_start = 0;
        let mut phdr_end = 0;

        let hdr_size = ELF64_HDR_SIZE;
        let elf_hdr =
            Elf::parse_header(&elf_buf).map_err(|e| errno!(ENOEXEC, "invalid ELF header"))?;

        // executables built with -fPIE are type ET_DYN (shared object file)
        if elf_hdr.e_type != ET_DYN {
            return_errno!(ENOEXEC, "ELF is not position-independent");
        }

        if elf_hdr.e_phnum == 0 {
            return_errno!(ENOEXEC, "ELF doesn't have any program segments");
        }

        let program_hdr_table_size = elf_hdr.e_phnum * elf_hdr.e_phentsize;
        inode.read_at(
            elf_hdr.e_phoff as usize,
            &mut elf_buf[hdr_size..hdr_size + (program_hdr_table_size as usize)],
        )?;
        Ok(elf_hdr)
    }

    // An offset to be subtracted from ELF vaddr for PIE
    pub fn base_load_address_offset(&self) -> u64 {
        let phdr = self.program_headers().nth(0).unwrap();
        phdr.p_vaddr - phdr.p_offset
    }
}

pub trait ProgramHeaderExt<'a> {
    fn loadable(&self) -> bool;
    fn is_interpreter(&self) -> bool;
    fn validate(&self) -> Result<()>;
    fn get_content(&self, elf_file: &ElfFile<'a>) -> &'a [u8];
}

impl<'a> ProgramHeaderExt<'a> for ProgramHeader {
    /// Is the segment loadable?
    fn loadable(&self) -> bool {
        let type_ = self.p_type;
        type_ == goblin::elf::program_header::PT_LOAD
    }

    fn is_interpreter(&self) -> bool {
        let type_ = self.p_type;
        type_ == goblin::elf::program_header::PT_INTERP
    }

    fn get_content(&self, elf_file: &ElfFile<'a>) -> &'a [u8] {
        let file_range = self.file_range();
        &elf_file.elf_buf[file_range.start..file_range.end]
    }

    /// Do some basic sanity checks in case the ELF is corrupted somehow
    fn validate(&self) -> Result<()> {
        if !self.p_align.is_power_of_two() {
            return_errno!(EINVAL, "invalid memory alignment");
        }

        if self.p_memsz < self.p_filesz {
            return_errno!(EINVAL, "memory size must be no less than file size");
        }
        Ok(())
    }
}
