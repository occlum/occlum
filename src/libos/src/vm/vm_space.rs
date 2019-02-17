use super::*;

impl VMSpace {
    pub unsafe fn new(
        addr: usize,
        size: usize,
        guard_type: VMGuardAreaType,
    ) -> Result<VMSpace, Error> {
        let range = unsafe { VMRange::new(addr, addr + size, VMGrowthType::Fixed)? };
        Ok(VMSpace { range, guard_type })
    }

    pub fn get_guard_type(&self) -> VMGuardAreaType {
        self.guard_type
    }

    pub fn alloc_domain(&mut self, size: usize) -> Result<VMDomain, Error> {
        let mut options = VMAllocOptions::new(size)?;
        options.growth(VMGrowthType::Upward)?;

        let new_range = self.range.alloc_subrange(&options)?;
        Ok(VMDomain { range: new_range })
    }

    pub fn dealloc_domain(&mut self, domain: &mut VMDomain) {
        self.range.dealloc_subrange(&mut domain.range)
    }

    pub fn resize_domain(&mut self, domain: &mut VMDomain, new_size: usize) -> Result<(), Error> {
        let options = VMResizeOptions::new(new_size)?;
        self.range.resize_subrange(&mut domain.range, &options)
    }
}
