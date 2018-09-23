/// Virtuam Memory Area (VMA)

use xmas_elf::program;
use xmas_elf::program::{ProgramHeader};
use std;
use std::sync::Arc;
use mm::MemObj;
use sgx_types::*;

#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct Vma {
    /// Basic info
    pub mem_size: usize,
    pub mem_align: usize,
    pub mem_flags: Perms,

    /// File mapping
    pub file_is_mapped: bool,
    pub mem_addr: usize,
    pub file_offset: usize,
    pub file_size: usize,

    /// Memory allocation
    pub mem_begin: usize,
    pub mem_end: usize,
    underlying: Arc<MemObj>,
}

const VMA_MIN_MEM_ALIGN: usize = (4 * 1024);

impl Vma {
    pub fn from_program_header<'a>(ph: &ProgramHeader<'a>)
        -> Result<Vma, &'static str>
    {
        let ph64 = match ph {
            ProgramHeader::Ph32(ph) => {
                return Err("Not support 32-bit ELF")
            }
            ProgramHeader::Ph64(ph64) => {
                ph64
            }
        };
        if ph64.align > 1 && ((ph64.offset % ph64.align) !=
                              (ph64.virtual_addr % ph64.align)) {
            return Err("Memory address and file offset is not equal, per modulo");
        }
        if ph64.mem_size < ph64.file_size {
            return Err("Memory size must be greater than file size");
        }

        let mut new_vma = Vma::new(ph64.mem_size as usize,
                                   ph64.align as usize,
                                   Perms::from(&ph64.flags))?;

        new_vma.mem_addr = ph64.virtual_addr as usize;
        new_vma.file_is_mapped = true;
        new_vma.file_offset = ph64.offset as usize;
        new_vma.file_size = ph64.file_size as usize;

        Ok(new_vma)
    }

    pub fn new(mem_size: usize, mem_align: usize, mem_flags: Perms)
        -> Result<Self, &'static str>
    {
        if mem_align == 0 || mem_align % VMA_MIN_MEM_ALIGN != 0 {
            return Err("Memory alignment is not a multiple of 4KB");
        }
        if mem_size == 0 {
            return Err("Memory size must be greater than zero");
        }

        Ok(Vma {
            mem_size: mem_size,
            mem_align: mem_align,
            mem_flags: mem_flags,
            .. Default::default()
        })
    }

    pub fn contains(&self, mem_addr: usize) -> bool {
        self.mem_begin <= mem_addr && mem_addr <= self.mem_end
    }
}

pub fn malloc_batch(vma_list: &mut [&mut Vma], mapped_data: &[u8])
    -> Result<usize, &'static str>
{
    let mut max_align = VMA_MIN_MEM_ALIGN;
    let mut total_size = 0;
    for vma in vma_list.into_iter() {
        let mem_begin = align_up(total_size, vma.mem_align);
        let mem_end = mem_begin + align_up(vma.mem_size, vma.mem_align);

        if vma.file_is_mapped {
            if vma.mem_addr < mem_begin ||
                vma.mem_addr + vma.mem_size > mem_end {
                    return Err("Impossible memory layout for the VMA");
                }
            if vma.file_offset > mapped_data.len() ||
                vma.file_offset + vma.file_size > mapped_data.len() {
                    return Err("Impossible to load data from file");
                }
        }

        total_size = mem_end;
        if vma.mem_align > max_align {
            max_align = vma.mem_align;
        }
    }

    let memobj = Arc::new(MemObj::new(total_size, max_align)?);
    let program_base_addr = memobj.get_addr();
    let mut mem_cur = program_base_addr;
    for vma in vma_list.into_iter() {
        vma.mem_begin = align_up(mem_cur, vma.mem_align);
        vma.mem_end = vma.mem_begin + align_up(vma.mem_size, vma.mem_align);
        vma.mem_addr += program_base_addr;
        vma.underlying = memobj.clone();

        if vma.file_is_mapped {
            let mut vma_data = unsafe {
                std::slice::from_raw_parts_mut(vma.mem_addr as *mut u8, vma.file_size)
            };
            vma_data.copy_from_slice(&mapped_data[vma.file_offset..
                vma.file_offset + vma.file_size]);
        }

        mem_cur = vma.mem_end;
    }

    Ok(program_base_addr)
}

pub fn mprotect_batch(vma_list: &[&Vma])
    -> Result<(), &'static str>
{
    for vma in vma_list.into_iter() {
        // If don't need to change memory permissions
        if vma.mem_flags == Perms(PERM_R | PERM_W) {
            continue;
        }

        let start = align_down(vma.mem_addr, 4096);
        let size = align_up(vma.mem_size, 4096);
        let perms = vma.mem_flags.0 as uint64_t;
        let status = unsafe {
            //TODO: use proper permissions
            //TODO: reset the permissions when drop VMA
            //trts_mprotect(start, size, perms)
            //println!("trts_mprotect: start = {}, size = {}", start, size);
            trts_mprotect(start, size, (PERM_R | PERM_W | PERM_X) as uint64_t)
        };
        if (status != sgx_status_t::SGX_SUCCESS) {
            return Err("trts_mprotect failed");
        }
    }
    Ok(())
}


#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Perms(pub u32);

pub const PERM_R: u32 = 0x1;
pub const PERM_W: u32 = 0x2;
pub const PERM_X: u32 = 0x4;

impl Perms {
    pub fn is_execute(&self) -> bool {
        self.0 & PERM_X == PERM_X
    }

    pub fn is_write(&self) -> bool {
        self.0 & PERM_W == PERM_W
    }

    pub fn is_read(&self) -> bool {
        self.0 & PERM_R == PERM_R
    }
}

impl<'a> From<&'a program::Flags> for Perms {
    fn from(flags: &'a program::Flags) -> Self {
        let mut val = 0;
        if flags.is_execute() { val |= PERM_X; }
        if flags.is_read() { val |= PERM_R; }
        if flags.is_write() { val |= PERM_W; }
        Perms(val)
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + (align - 1)) / align * align
}

fn align_down(addr: usize, align: usize) -> usize {
    addr & !(align - 1)
}

#[link(name = "sgx_trts")]
extern {
    pub fn trts_mprotect(start: size_t, size: size_t, perms: uint64_t) -> sgx_status_t;
}
