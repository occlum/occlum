use fs::FileDesc;
use prelude::*;
use process::{get_current, Process, ProcessRef};
use std::fmt;

#[macro_use]
mod vm_range;
mod process_vm;
mod vm_area;

pub use self::process_vm::ProcessVM;
pub use self::vm_area::{
    VMArea, VMAreaFlags, VMDomain, VMSpace, VM_AREA_FLAG_R, VM_AREA_FLAG_W, VM_AREA_FLAG_X,
};
pub use self::vm_range::{VMRange, VMRangeTrait};

// TODO: separate proc and flags
// TODO: accept fd and offset
pub fn do_mmap(addr: usize, size: usize, flags: VMAreaFlags) -> Result<usize, Error> {
    info!(
        "mmap: addr: {:#x}, size: {:#x}, flags: {:?}",
        addr, size, flags
    );
    let current_ref = get_current();
    let current_process = current_ref.lock().unwrap();
    let current_vm_ref = current_process.get_vm();
    let mut current_vm = current_vm_ref.lock().unwrap();
    current_vm.mmap(addr, size, flags)
}

pub fn do_munmap(addr: usize, size: usize) -> Result<(), Error> {
    info!("munmap: addr: {:#x}, size: {:#x}", addr, size);
    let current_ref = get_current();
    let current_process = current_ref.lock().unwrap();
    let current_vm_ref = current_process.get_vm();
    let mut current_vm = current_vm_ref.lock().unwrap();
    current_vm.munmap(addr, size)
}

// TODO: accept flags
pub fn do_mremap(
    old_addr: usize,
    old_size: usize,
    options: &VMResizeOptions,
) -> Result<usize, Error> {
    info!(
        "mremap: oldaddr: {:#x}, oldsize: {:#x}, options: {:?}",
        old_addr, old_size, options
    );
    let current_ref = get_current();
    let current_process = current_ref.lock().unwrap();
    let current_vm_ref = current_process.get_vm();
    let mut current_vm = current_vm_ref.lock().unwrap();
    current_vm.mremap(old_addr, old_size, options)
}

pub fn do_brk(addr: usize) -> Result<usize, Error> {
    info!("brk: addr: {:#x}", addr);
    let current_ref = get_current();
    let current_process = current_ref.lock().unwrap();
    let current_vm_ref = current_process.get_vm();
    let mut current_vm = current_vm_ref.lock().unwrap();
    current_vm.brk(addr)
}

pub const PAGE_SIZE: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VMGuardAreaType {
    None,
    Static { size: usize, align: usize },
    Dynamic { size: usize },
}

#[derive(Clone, PartialEq, Default)]
pub struct VMAllocOptions {
    size: usize,
    addr: VMAddrOption,
    growth: VMGrowthType,
    description: String,
    fill_zeros: bool,
}

impl VMAllocOptions {
    pub fn new(size: usize) -> Result<VMAllocOptions, Error> {
        if size % PAGE_SIZE != 0 {
            return errno!(EINVAL, "Size is not page-aligned");
        }
        Ok(VMAllocOptions {
            size,
            ..Default::default()
        })
    }

    pub fn addr(&mut self, addr: VMAddrOption) -> Result<&mut Self, Error> {
        if addr.is_addr_given() && addr.get_addr() % PAGE_SIZE != 0 {
            return errno!(EINVAL, "Invalid address");
        }
        self.addr = addr;
        Ok(self)
    }

    pub fn growth(&mut self, growth: VMGrowthType) -> Result<&mut Self, Error> {
        self.growth = growth;
        Ok(self)
    }

    pub fn description(&mut self, description: &str) -> Result<&mut Self, Error> {
        self.description = description.to_owned();
        Ok(self)
    }

    pub fn fill_zeros(&mut self, fill_zeros: bool) -> Result<&mut Self, Error> {
        self.fill_zeros = fill_zeros;
        Ok(self)
    }
}

impl fmt::Debug for VMAllocOptions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "VMAllocOptions {{ size: 0x{:X?}, addr: {:?}, growth: {:?} }}",
            self.size, self.addr, self.growth
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VMAddrOption {
    Any,           // Free to choose any address
    Hint(usize),   // Near the given address
    Fixed(usize),  // Must be the given address
    Beyond(usize), // Must be greater or equal to the given address
}

impl Default for VMAddrOption {
    fn default() -> VMAddrOption {
        VMAddrOption::Any
    }
}

impl VMAddrOption {
    pub fn is_addr_given(&self) -> bool {
        match self {
            VMAddrOption::Any => false,
            _ => true,
        }
    }

    pub fn get_addr(&self) -> usize {
        match self {
            VMAddrOption::Hint(addr) | VMAddrOption::Fixed(addr) | VMAddrOption::Beyond(addr) => {
                *addr
            }
            VMAddrOption::Any => panic!("No address given"),
        }
    }
}

/// How VMRange may grow:
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VMGrowthType {
    Fixed,
    Upward,   // e.g., mmaped regions grow upward
    Downward, // e.g., stacks grows downward
}

impl Default for VMGrowthType {
    fn default() -> VMGrowthType {
        VMGrowthType::Fixed
    }
}

#[derive(Clone, Debug, Default)]
pub struct VMResizeOptions {
    new_size: usize,
    new_addr: VMAddrOption,
    fill_zeros: bool,
}

impl VMResizeOptions {
    pub fn new(new_size: usize) -> Result<VMResizeOptions, Error> {
        if new_size % PAGE_SIZE != 0 {
            return errno!(EINVAL, "Size is not page-aligned");
        }
        Ok(VMResizeOptions {
            new_size,
            ..Default::default()
        })
    }

    pub fn addr(&mut self, new_addr: VMAddrOption) -> &mut Self {
        self.new_addr = new_addr;
        self
    }

    pub fn fill_zeros(&mut self, fill_zeros: bool) -> &mut Self {
        self.fill_zeros = fill_zeros;
        self
    }
}
