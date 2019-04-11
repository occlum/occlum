use super::*;

#[derive(Debug)]
pub struct VMSpace {
    range: VMRange,
    guard_type: VMGuardAreaType,
}

impl_vmrange_trait_for!(VMSpace, range);

impl VMSpace {
    pub unsafe fn new(
        addr: usize,
        size: usize,
        guard_type: VMGuardAreaType,
        desc: &str,
    ) -> Result<VMSpace, Error> {
        let addr = align_up(addr, PAGE_SIZE);
        let size = align_down(size, PAGE_SIZE);
        let range = unsafe { VMRange::new(addr, addr + size, VMGrowthType::Fixed, desc)? };
        Ok(VMSpace { range, guard_type })
    }

    pub fn get_guard_type(&self) -> VMGuardAreaType {
        self.guard_type
    }

    pub fn alloc_domain(&mut self, size: usize, desc: &str) -> Result<VMDomain, Error> {
        let mut options = VMAllocOptions::new(size)?;
        options.growth(VMGrowthType::Upward)?
            .description(desc)?;

        let new_range = self.range.alloc_subrange(&options)?;
        Ok(VMDomain { range: new_range })
    }

    pub fn dealloc_domain(&mut self, mut domain: VMDomain) {
        self.range.dealloc_subrange(&mut domain.range)
    }

    pub fn resize_domain(&mut self, domain: &mut VMDomain, new_size: usize) -> Result<(), Error> {
        let options = VMResizeOptions::new(new_size)?;
        self.range.resize_subrange(&mut domain.range, &options)
    }
}


#[derive(Debug)]
pub struct VMDomain {
    range: VMRange,
}

impl_vmrange_trait_for!(VMDomain, range);

impl VMDomain {
    pub fn alloc_area(
        &mut self,
        options: &VMAllocOptions,
        flags: VMAreaFlags,
    ) -> Result<VMArea, Error> {
        let new_range = self.range.alloc_subrange(options)?;

        // Init the memory area with all zeros
        unsafe {
            let mem_ptr = new_range.get_start() as *mut c_void;
            let mem_size = new_range.get_size() as size_t;
            memset(mem_ptr, 0 as c_int, mem_size);
        }

        Ok(VMArea {
            range: new_range,
            flags: flags,
        })
    }

    pub fn dealloc_area(&mut self, mut area: VMArea) {
        self.range.dealloc_subrange(&mut area.range)
    }

    pub fn resize_area(
        &mut self,
        area: &mut VMArea,
        options: &VMResizeOptions,
    ) -> Result<(), Error> {
        // TODO: init memory with zeros when expanding!
        self.range.resize_subrange(&mut area.range, options)
    }
}

#[link(name = "sgx_tstdc")]
extern "C" {
    pub fn memset(p: *mut c_void, c: c_int, n: size_t) -> *mut c_void;
}


#[derive(Debug)]
pub struct VMArea {
    range: VMRange,
    flags: VMAreaFlags,
}

impl_vmrange_trait_for!(VMArea, range);

impl VMArea {
    pub fn get_flags(&self) -> &VMAreaFlags {
        &self.flags
    }

    pub fn get_flags_mut(&mut self) -> &mut VMAreaFlags {
        &mut self.flags
    }
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
