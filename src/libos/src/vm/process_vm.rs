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
            match VMSpace::new(addr, size, VMGuardAreaType::None, "DATA_SPACE") {
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
    data_domain: Option<Box<VMDomain>>,
    code_vma: Option<Box<VMArea>>,
    data_vma: Option<Box<VMArea>>,
    heap_vma: Option<Box<VMArea>>,
    stack_vma: Option<Box<VMArea>>,
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
        // Allocate the data domain from the global data space
        let mut data_domain = {
            let data_domain_size = code_size + data_size + heap_size + stack_size + mmap_size;
            let data_domain = DATA_SPACE
                .lock()
                .unwrap()
                .alloc_domain(data_domain_size, "data_domain")?;
            data_domain
        };
        // Allocate vmas from the data domain
        let (code_vma, data_vma, heap_vma, stack_vma) = match ProcessVM::alloc_vmas(
            &mut data_domain,
            code_size,
            data_size,
            heap_size,
            stack_size,
        ) {
            Err(e) => {
                // Note: we need to handle error here so that we can
                // deallocate the data domain explictly.
                DATA_SPACE.lock().unwrap().dealloc_domain(data_domain);
                return Err(e);
            }
            Ok(vmas) => vmas,
        };
        // Initial value of the program break
        let brk = heap_vma.get_start();
        // No mmapped vmas initially
        let mmap_vmas = Vec::new();

        let vm = ProcessVM {
            data_domain: Some(Box::new(data_domain)),
            code_vma: Some(Box::new(code_vma)),
            data_vma: Some(Box::new(data_vma)),
            heap_vma: Some(Box::new(heap_vma)),
            stack_vma: Some(Box::new(stack_vma)),
            mmap_vmas: mmap_vmas,
            brk: brk,
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
            |addr: &mut usize, desc, size, flags, growth, fill_zeros| -> Result<_, Error> {
                let mut options = VMAllocOptions::new(size)?;
                options
                    .addr(VMAddrOption::Fixed(*addr))?
                    .growth(growth)?
                    .description(desc)?
                    .fill_zeros(fill_zeros)?;
                let new_vma = data_domain.alloc_area(&options, flags)?;
                *addr += size;
                Ok(new_vma)
            };

        let rx_flags = VMAreaFlags(VM_AREA_FLAG_R | VM_AREA_FLAG_X);
        let rw_flags = VMAreaFlags(VM_AREA_FLAG_R | VM_AREA_FLAG_W);

        let code_vma = alloc_vma_continuously(
            &mut addr,
            "code_vma",
            code_size,
            rx_flags,
            VMGrowthType::Fixed,
            !cfg!(feature = "integrity_only_opt"),
        )?;
        let data_vma = alloc_vma_continuously(
            &mut addr,
            "data_vma",
            data_size,
            rw_flags,
            VMGrowthType::Fixed,
            !cfg!(feature = "integrity_only_opt"),
        )?;
        let heap_vma = alloc_vma_continuously(
            &mut addr,
            "heap_vma",
            0,
            rw_flags,
            VMGrowthType::Upward,
            true,
        )?;
        // Preserve the space for heap
        addr += heap_size;
        // After the heap is the stack
        let stack_vma = alloc_vma_continuously(
            &mut addr,
            "stack_vma",
            stack_size,
            rw_flags,
            VMGrowthType::Downward,
            false,
        )?;
        Ok((code_vma, data_vma, heap_vma, stack_vma))
    }

    pub fn get_base_addr(&self) -> usize {
        self.get_code_vma().get_start()
    }

    pub fn get_code_vma(&self) -> &VMArea {
        &self.code_vma.as_ref().unwrap()
    }

    pub fn get_data_vma(&self) -> &VMArea {
        &self.data_vma.as_ref().unwrap()
    }

    pub fn get_heap_vma(&self) -> &VMArea {
        &self.heap_vma.as_ref().unwrap()
    }

    pub fn get_stack_vma(&self) -> &VMArea {
        &self.stack_vma.as_ref().unwrap()
    }

    pub fn get_stack_top(&self) -> usize {
        self.get_stack_vma().get_end()
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
                        return errno!(EINVAL, "Beyond valid memory range");
                    }
                    // TODO: Fixed or Hint? Should hanle mmap flags
                    VMAddrOption::Hint(addr)
                })?
                .growth(VMGrowthType::Upward)?;
            alloc_options
        };
        // TODO: when failed, try to resize data_domain
        let new_mmap_vma = self
            .get_data_domain_mut()
            .alloc_area(&alloc_options, flags)?;
        let addr = new_mmap_vma.get_start();
        self.mmap_vmas.push(Box::new(new_mmap_vma));
        Ok(addr)
    }

    // TODO: handle the case when the given range [addr, addr + size)
    // does not match exactly with any vma. For example, when this range
    // cover multiple ranges or cover some range partially.
    pub fn munmap(&mut self, addr: usize, size: usize) -> Result<(), Error> {
        let mmap_vma_i = {
            let mmap_vma_i = self
                .get_mmap_vmas()
                .iter()
                .position(|vma| vma.get_start() == addr && vma.get_end() == addr + size);
            if mmap_vma_i.is_none() {
                return errno!(EINVAL, "memory area not found");
            }
            mmap_vma_i.unwrap()
        };

        let removed_mmap_vma = self.mmap_vmas.swap_remove(mmap_vma_i);
        self.get_data_domain_mut()
            .dealloc_area(unbox(removed_mmap_vma));
        Ok(())
    }

    pub fn mremap(
        &mut self,
        old_addr: usize,
        old_size: usize,
        options: &VMResizeOptions,
    ) -> Result<usize, Error> {
        // TODO: Implement this!
        errno!(EINVAL, "Not implemented")
    }

    pub fn brk(&mut self, new_brk: usize) -> Result<usize, Error> {
        let (heap_start, heap_end) = {
            let heap_vma = self.heap_vma.as_ref().unwrap();
            (heap_vma.get_start(), heap_vma.get_end())
        };
        if new_brk == 0 {
            return Ok(self.get_brk());
        } else if new_brk < heap_start {
            return errno!(EINVAL, "New brk address is too low");
        } else if new_brk > heap_end {
            let resize_options = {
                let new_heap_end = align_up(new_brk, PAGE_SIZE);
                let new_heap_size = new_heap_end - heap_start;
                let mut options = VMResizeOptions::new(new_heap_size)?;
                options
                    .addr(VMAddrOption::Fixed(heap_start))
                    .fill_zeros(true);
                options
            };
            let heap_vma = self.heap_vma.as_mut().unwrap();
            let data_domain = self.data_domain.as_mut().unwrap();
            data_domain.resize_area(heap_vma, &resize_options)?;
        }
        self.brk = new_brk;
        return Ok(new_brk);
    }

    fn get_data_domain_mut(&mut self) -> &mut Box<VMDomain> {
        self.data_domain.as_mut().unwrap()
    }
}

impl Drop for ProcessVM {
    fn drop(&mut self) {
        // Remove all vma from the domain
        {
            let data_domain = self.data_domain.as_mut().unwrap();
            data_domain.dealloc_area(unbox(self.code_vma.take().unwrap()));
            data_domain.dealloc_area(unbox(self.data_vma.take().unwrap()));
            data_domain.dealloc_area(unbox(self.heap_vma.take().unwrap()));
            data_domain.dealloc_area(unbox(self.stack_vma.take().unwrap()));
            for mmap_vma in self.mmap_vmas.drain(..) {
                data_domain.dealloc_area(unbox(mmap_vma));
            }
        }

        // Remove the domain from its parent space
        DATA_SPACE
            .lock()
            .unwrap()
            .dealloc_domain(unbox(self.data_domain.take().unwrap()));
    }
}
