use fs::{off_t, FileDesc};
use prelude::*;
use process::{get_current, Process, ProcessRef};
use std::fmt;

// TODO: Rename VMSpace to VMUniverse

#[macro_use]
mod vm_range;
mod process_vm;
mod vm_area;
mod vm_domain;
mod vm_space;

pub use self::process_vm::ProcessVM;
pub use self::vm_range::{VMRange, VMRangeTrait};

// TODO: separate proc and flags
// TODO: accept fd and offset
pub fn do_mmap(addr: usize, size: usize, flags: VMAreaFlags) -> Result<usize, Error> {
    let current_ref = get_current();
    let mut current_process = current_ref.lock().unwrap();
    let current_vm = current_process.get_vm_mut();
    current_vm.mmap(addr, size, flags)
}

pub fn do_munmap(addr: usize, size: usize) -> Result<(), Error> {
    let current_ref = get_current();
    let mut current_process = current_ref.lock().unwrap();
    let current_vm = current_process.get_vm_mut();
    current_vm.munmap(addr, size)
}

// TODO: accept flags
pub fn do_mremap(
    old_addr: usize,
    old_size: usize,
    options: &VMResizeOptions,
) -> Result<usize, Error> {
    let current_ref = get_current();
    let mut current_process = current_ref.lock().unwrap();
    let current_vm = current_process.get_vm_mut();
    current_vm.mremap(old_addr, old_size, options)
}

pub fn do_brk(addr: usize) -> Result<usize, Error> {
    let current_ref = get_current();
    let mut current_process = current_ref.lock().unwrap();
    let current_vm = current_process.get_vm_mut();
    current_vm.brk(addr)
}

pub const PAGE_SIZE: usize = 4096;

#[derive(Debug)]
pub struct VMSpace {
    range: VMRange,
    guard_type: VMGuardAreaType,
}

#[derive(Debug, Default)]
pub struct VMDomain {
    range: VMRange,
}

#[derive(Debug, Default)]
pub struct VMArea {
    range: VMRange,
    flags: VMAreaFlags,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VMGuardAreaType {
    None,
    Static { size: usize, align: usize },
    Dynamic { size: usize },
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct VMAreaFlags(pub u32);

pub const VM_AREA_FLAG_R: u32 = 0x1;
pub const VM_AREA_FLAG_W: u32 = 0x2;
pub const VM_AREA_FLAG_X: u32 = 0x4;

impl VMAreaFlags {
    pub fn can_execute(&self) -> bool {
        self.0 & VM_AREA_FLAG_X == VM_AREA_FLAG_X
    }

    pub fn can_write(&self) -> bool {
        self.0 & VM_AREA_FLAG_W == VM_AREA_FLAG_W
    }

    pub fn can_read(&self) -> bool {
        self.0 & VM_AREA_FLAG_R == VM_AREA_FLAG_R
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct VMAllocOptions {
    size: usize,
    addr: VMAddrOption,
    growth: Option<VMGrowthType>,
}

impl VMAllocOptions {
    pub fn new(size: usize) -> Result<VMAllocOptions, Error> {
        if size % PAGE_SIZE != 0 {
            return Err(Error::new(Errno::EINVAL, "Size is not page-aligned"));
        }
        Ok(VMAllocOptions {
            size,
            ..Default::default()
        })
    }

    pub fn addr(&mut self, addr: VMAddrOption) -> Result<&mut Self, Error> {
        if addr.is_addr_given() && addr.get_addr() % PAGE_SIZE != 0 {
            return Err(Error::new(Errno::EINVAL, "Invalid address"));
        }
        self.addr = addr;
        Ok(self)
    }

    pub fn growth(&mut self, growth: VMGrowthType) -> Result<&mut Self, Error> {
        self.growth = Some(growth);
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

impl Default for VMAllocOptions {
    fn default() -> VMAllocOptions {
        VMAllocOptions {
            size: 0,
            addr: VMAddrOption::Any,
            growth: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VMAddrOption {
    Any,           // Free to choose any address
    Hint(usize),   // Near the given address
    Fixed(usize),  // Must be the given address
    Beyond(usize), // Must be greater or equal to the given address
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
    Upward,   // e.g., mmaped regions grow upward
    Downward, // e.g., stacks grows downward
    Fixed,
}

#[derive(Clone, Debug)]
pub struct VMResizeOptions {
    new_size: usize,
    new_addr: Option<VMAddrOption>,
}

impl VMResizeOptions {
    pub fn new(new_size: usize) -> Result<VMResizeOptions, Error> {
        if new_size % PAGE_SIZE != 0 {
            return Err(Error::new(Errno::EINVAL, "Size is not page-aligned"));
        }
        Ok(VMResizeOptions {
            new_size,
            ..Default::default()
        })
    }

    pub fn addr(&mut self, new_addr: VMAddrOption) -> &mut Self {
        self.new_addr = Some(new_addr);
        self
    }
}

impl Default for VMResizeOptions {
    fn default() -> VMResizeOptions {
        VMResizeOptions {
            new_size: 0,
            new_addr: None,
        }
    }
}
