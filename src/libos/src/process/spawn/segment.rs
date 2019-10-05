use super::*;
use std::slice;
use xmas_elf::program::ProgramHeader;

#[derive(Debug, Default)]
pub struct Segment {
    // Static info from ELF
    mem_addr: usize,
    mem_align: usize,
    mem_size: usize,
    file_offset: usize,
    file_size: usize,
    // Runtime info after loaded
    runtime_base_addr: Option<usize>,
}

pub const PERM_R: u32 = 0x1;
pub const PERM_W: u32 = 0x2;
pub const PERM_X: u32 = 0x4;

impl Segment {
    pub fn get_mem_addr(&self) -> usize {
        self.mem_addr
    }
    pub fn get_mem_align(&self) -> usize {
        self.mem_align
    }
    pub fn get_mem_size(&self) -> usize {
        self.mem_size
    }

    pub fn from_program_header(ph: &ProgramHeader) -> Result<Segment> {
        let ph64 = match ph {
            ProgramHeader::Ph32(ph) => {
                return_errno!(ENOEXEC, "not support 32-bit ELF");
            }
            ProgramHeader::Ph64(ph64) => ph64,
        };
        if ph64.align > 1 && ((ph64.offset % ph64.align) != (ph64.virtual_addr % ph64.align)) {
            return_errno!(
                EINVAL,
                "memory address and file offset is not equal, per modulo"
            );
        }
        if ph64.mem_size < ph64.file_size {
            return_errno!(EINVAL, "memory size must be greater than file size");
        }
        if !ph64.align.is_power_of_two() {
            return_errno!(EINVAL, "memory alignment must be a power of two");
        }

        Ok(Segment {
            mem_addr: ph64.virtual_addr as usize,
            mem_align: ph64.align as usize,
            mem_size: ph64.mem_size as usize,
            file_offset: ph64.offset as usize,
            file_size: ph64.file_size as usize,
            ..Default::default()
        })
    }

    pub fn load_from_file(&self, elf_buf: &[u8]) {
        let mut target_buf = unsafe {
            slice::from_raw_parts_mut(
                (self.runtime_base_addr.unwrap() + self.mem_addr) as *mut u8,
                self.mem_size,
            )
        };
        target_buf[0..self.file_size]
            .copy_from_slice(&elf_buf[self.file_offset..(self.file_offset + self.file_size)]);
        #[cfg(feature = "integrity_only_opt")]
        for i in &mut target_buf[self.file_size..self.mem_size] {
            *i = 0;
        }
    }

    pub fn set_runtime_base(&mut self, runtime_base_addr: usize) {
        self.runtime_base_addr = Some(runtime_base_addr);
    }

    pub fn mprotect(&mut self, perm: u32) {
        panic!("Not implemented yet!");
        /*
        unsafe {
            trts_mprotect(self.start_addr, self.end_addr - self.start_addr,
                          perm as u64);
        }
        */
    }
}

pub fn get_code_segment(elf_file: &ElfFile) -> Result<Segment> {
    let code_ph = elf_helper::get_code_program_header(elf_file)
        .map_err(|e| errno!(ENOEXEC, "failed to get the program header of code"))?;
    Segment::from_program_header(&code_ph)
}

pub fn get_data_segment(elf_file: &ElfFile) -> Result<Segment> {
    let data_ph = elf_helper::get_data_program_header(elf_file)
        .map_err(|e| errno!(ENOEXEC, "failed to get the program header of code"))?;
    Segment::from_program_header(&data_ph)
}

#[link(name = "sgx_trts")]
extern "C" {
    // XXX: trts_mprotect is a private SGX function that is not supposed to be
    // used by external users. At least, this is the case for SGX v2.2. To use
    // this function, we need to modify Intel SGX SDK slightly. I suppose
    // this functionality will be exposed to external users as an SGX API in
    // the future.
    pub fn trts_mprotect(start: size_t, size: size_t, perms: uint64_t) -> sgx_status_t;
}
