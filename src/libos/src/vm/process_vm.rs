use super::*;

// TODO: examine the ProcessVM code for memory leakage

lazy_static! {
    static ref DATA_SPACE: SgxMutex<VMSpace> = {
        let (addr, size) = {
            let mut addr: usize = 0;
            let mut size: usize = 0;
            unsafe { vm_get_prealloced_data_space(&mut addr, &mut size) };
            (addr, size)
        };
        let vm_space = unsafe {
            match VMSpace::new(addr, size, VMGuardAreaType::None) {
                Ok(vm_space) => vm_space,
                Err(_) => panic!("Failed to create a VMSpace"),
            }
        };
        SgxMutex::new(vm_space)
    };
}

extern "C" {
    pub fn vm_get_prealloced_data_space(addr: &mut usize, size: &mut usize);
}

#[derive(Debug, Default)]
pub struct ProcessVM {
    //code_domain: VMDomain,
    data_domain: VMDomain,
    code_vma: VMArea,
    data_vma: VMArea,
    heap_vma: VMArea,
    stack_vma: VMArea,
    mmap_vmas: Vec<Box<VMArea>>,
    brk: usize,
}

impl ProcessVM {
    pub fn new(
        code_size: usize,
        data_size: usize,
        heap_size: usize,
        stack_size: usize,
        mmap_size: usize,
    ) -> Result<ProcessVM, Error> {
        let data_domain_size = code_size + data_size + heap_size + stack_size + mmap_size;
        let mut data_domain = DATA_SPACE.lock().unwrap().alloc_domain(data_domain_size)?;

        let (code_vma, data_vma, heap_vma, stack_vma) = ProcessVM::alloc_vmas(
            &mut data_domain,
            code_size,
            data_size,
            heap_size,
            stack_size,
        )?;
        // Initial value of the program break
        let brk = heap_vma.get_start();
        // No mmapped vmas initially
        let mmap_vmas = Vec::new();

        let vm = ProcessVM {
            data_domain,
            code_vma,
            data_vma,
            heap_vma,
            stack_vma,
            mmap_vmas,
            brk,
        };
        Ok(vm)
    }

    fn alloc_vmas(
        data_domain: &mut VMDomain,
        code_size: usize,
        data_size: usize,
        heap_size: usize,
        stack_size: usize,
    ) -> Result<(VMArea, VMArea, VMArea, VMArea), Error> {
        let mut addr = data_domain.get_start();

        let mut alloc_vma_continuously =
            |addr: &mut usize, size, flags, growth| -> Result<_, Error> {
                let mut options = VMAllocOptions::new(size)?;
                options.addr(VMAddrOption::Fixed(*addr))?.growth(growth)?;
                let new_vma = data_domain.alloc_area(&options, flags)?;
                *addr += size;
                Ok(new_vma)
            };

        let rx_flags = VMAreaFlags(VM_AREA_FLAG_R | VM_AREA_FLAG_X);
        let rw_flags = VMAreaFlags(VM_AREA_FLAG_R | VM_AREA_FLAG_W);

        let code_vma = alloc_vma_continuously(&mut addr, code_size, rx_flags, VMGrowthType::Fixed)?;
        let data_vma = alloc_vma_continuously(&mut addr, data_size, rw_flags, VMGrowthType::Fixed)?;
        let heap_vma = alloc_vma_continuously(&mut addr, 0, rw_flags, VMGrowthType::Upward)?;
        // Preserve the space for heap
        addr += heap_size;
        // After the heap is the stack
        let stack_vma =
            alloc_vma_continuously(&mut addr, stack_size, rw_flags, VMGrowthType::Downward)?;
        Ok((code_vma, data_vma, heap_vma, stack_vma))
    }

    pub fn get_base_addr(&self) -> usize {
        self.code_vma.get_start()
    }

    pub fn get_code_vma(&self) -> &VMArea {
        &self.code_vma
    }

    pub fn get_data_vma(&self) -> &VMArea {
        &self.data_vma
    }

    pub fn get_heap_vma(&self) -> &VMArea {
        &self.heap_vma
    }

    pub fn get_stack_vma(&self) -> &VMArea {
        &self.stack_vma
    }

    pub fn get_stack_top(&self) -> usize {
        self.stack_vma.get_end()
    }

    pub fn get_mmap_vmas(&self) -> &[Box<VMArea>] {
        &self.mmap_vmas[..]
    }

    pub fn get_brk_start(&self) -> usize {
        self.get_heap_vma().get_start()
    }

    pub fn get_brk(&self) -> usize {
        self.brk
    }

    pub fn get_mmap_start(&self) -> usize {
        self.get_stack_vma().get_end()
    }

    // TODO: support overriding the mmaping of already mmaped range
    pub fn mmap(&mut self, addr: usize, size: usize, flags: VMAreaFlags) -> Result<usize, Error> {
        let alloc_options = {
            let mmap_start_addr = self.get_mmap_start();

            let mut alloc_options = VMAllocOptions::new(size)?;
            alloc_options
                .addr(if addr == 0 {
                    VMAddrOption::Beyond(mmap_start_addr)
                } else {
                    if addr < mmap_start_addr {
                        return Err(Error::new(Errno::EINVAL, "Beyond valid memory range"));
                    }
                    VMAddrOption::Fixed(addr)
                })?
                .growth(VMGrowthType::Upward)?;
            alloc_options
        };
        // TODO: when failed, try to resize data_domain
        let new_mmap_vma = self.data_domain.alloc_area(&alloc_options, flags)?;
        let addr = new_mmap_vma.get_start();
        self.mmap_vmas.push(Box::new(new_mmap_vma));
        Ok(addr)
    }

    pub fn munmap(&mut self, addr: usize, size: usize) -> Result<(), Error> {
        // TODO: handle the case when the given range [addr, addr + size)
        // does not match exactly with any vma. For example, when this range
        // cover multiple ranges or cover some range partially.

        let mmap_vma_i = {
            let mmap_vma_i = self
                .get_mmap_vmas()
                .iter()
                .position(|vma| vma.get_start() == addr && vma.get_end() == addr + size);
            if mmap_vma_i.is_none() {
                return Ok(());
            }
            mmap_vma_i.unwrap()
        };

        let mut removed_mmap_vma = self.mmap_vmas.swap_remove(mmap_vma_i);
        self.data_domain.dealloc_area(&mut removed_mmap_vma);
        Ok(())
    }

    pub fn mremap(
        &mut self,
        old_addr: usize,
        old_size: usize,
        options: &VMResizeOptions,
    ) -> Result<usize, Error> {
        // TODO: Implement this!
        Err(Error::new(Errno::EINVAL, "Not implemented"))
    }

    pub fn brk(&mut self, new_brk: usize) -> Result<usize, Error> {
        if new_brk == 0 {
            return Ok(self.get_brk());
        } else if new_brk < self.heap_vma.get_start() {
            return errno!(EINVAL, "New brk address is too low");
        } else if new_brk <= self.heap_vma.get_end() {
            self.brk = new_brk;
            return Ok(new_brk);
        }

        // TODO: init the memory with zeros for the expanded area
        let resize_options = {
            let brk_start = self.get_brk_start();
            let new_heap_size = align_up(new_brk, 4096) - brk_start;
            let mut options = VMResizeOptions::new(new_heap_size)?;
            options.addr(VMAddrOption::Fixed(brk_start));
            options
        };
        self.data_domain
            .resize_area(&mut self.heap_vma, &resize_options)?;
        Ok(new_brk)
    }
}

impl Drop for ProcessVM {
    fn drop(&mut self) {
        let data_domain = &mut self.data_domain;

        // Remove all vma from the domain
        data_domain.dealloc_area(&mut self.code_vma);
        data_domain.dealloc_area(&mut self.data_vma);
        data_domain.dealloc_area(&mut self.heap_vma);
        data_domain.dealloc_area(&mut self.stack_vma);
        for mmap_vma in &mut self.mmap_vmas {
            data_domain.dealloc_area(mmap_vma);
        }

        // Remove the domain from its parent space
        DATA_SPACE.lock().unwrap().dealloc_domain(data_domain);
    }
}
