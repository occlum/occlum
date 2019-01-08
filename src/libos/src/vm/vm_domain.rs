use super::*;

impl VMDomain {
    pub fn alloc_area(&mut self, options: &VMAllocOptions, flags: VMAreaFlags) -> Result<VMArea, Error> {
        let new_range = self.range.alloc_subrange(options)?;

        // Init the memory area with all zeros
        unsafe {
            let mem_ptr = new_range.get_start() as *mut c_void;
            let mem_size = new_range.get_size() as size_t;
            memset(mem_ptr, 0 as c_int, mem_size);
        }

        Ok(VMArea { range: new_range, flags: flags })
    }

    pub fn dealloc_area(&mut self, area: &mut VMArea) {
        self.range.dealloc_subrange(&mut area.range)
    }

    pub fn resize_area(&mut self, area: &mut VMArea, options: &VMResizeOptions)
        -> Result<(), Error>
    {
        // TODO: init memory with zeros when expanding!
        self.range.resize_subrange(&mut area.range, options)
    }
}

#[link(name = "sgx_tstdc")]
extern {
    pub fn memset(p: *mut c_void, c: c_int, n: size_t) -> *mut c_void;
}
