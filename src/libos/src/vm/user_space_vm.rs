use super::*;
use super::vm_manager::{VMRange, VMManager, VMMapOptionsBuilder, VMMapOptions};

/// The virtual memory manager for the entire user space
#[derive(Debug)]
pub struct UserSpaceVMManager {
    vm_manager: Arc<SgxMutex<VMManager>>,
}

impl UserSpaceVMManager {
    pub unsafe fn from(addr: usize, size: usize) -> Result<UserSpaceVMManager, Error> {
        let vm_manager = Arc::new(SgxMutex::new(VMManager::from(addr, size)?));
        Ok(UserSpaceVMManager {
            vm_manager,
        })
    }

    pub fn alloc(&self, size: usize) -> Result<UserSpaceVMRange, Error> {
        let user_vm_range = unsafe {
            let mmap_options = VMMapOptionsBuilder::default()
                .size(size)
                .build()?;

            let mut vm_manager = self.vm_manager.lock().unwrap();
            let user_vm_addr = vm_manager.mmap(&mmap_options)?;
            VMRange::from_unchecked(user_vm_addr, user_vm_addr + size)
        };
        Ok(UserSpaceVMRange::new(user_vm_range, self.vm_manager.clone()))
    }

    pub fn alloc_dummy(&self) -> UserSpaceVMRange {
        let empty_user_vm_range = unsafe {
            VMRange::from_unchecked(0, 0)
        };
        UserSpaceVMRange::new(empty_user_vm_range, self.vm_manager.clone())
    }
}

lazy_static! {
    pub static ref USER_SPACE_VM_MANAGER: UserSpaceVMManager = {
        let (addr, size) = {
            let mut addr: usize = 0;
            let mut size: usize = 0;
            unsafe { vm_get_preallocated_user_space_memory(&mut addr, &mut size) };
            (addr, size)
        };
        let user_space_vm_manager = unsafe {
            match UserSpaceVMManager::from(addr, size) {
                Ok(user_space_vm) => user_space_vm,
                Err(_) => panic!("Failed to initialize the user space virtual memory"),
            }
        };
        user_space_vm_manager
    };
}

extern "C" {
    pub fn vm_get_preallocated_user_space_memory(addr: &mut usize, size: &mut usize);
}


#[derive(Debug)]
pub struct UserSpaceVMRange {
    vm_range: VMRange,
    vm_manager: Arc<SgxMutex<VMManager>>,
}

impl UserSpaceVMRange {
    fn new(vm_range: VMRange, vm_manager: Arc<SgxMutex<VMManager>>) -> UserSpaceVMRange {
        UserSpaceVMRange {
            vm_range,
            vm_manager,
        }
    }

    pub fn range(&self) -> &VMRange {
        &self.vm_range
    }
}

impl Drop for UserSpaceVMRange {
    fn drop(&mut self) {
        let addr = self.vm_range.start();
        let size = self.vm_range.size();
        if size == 0 { return; }
        let mut vm_manager = self.vm_manager.lock().unwrap();
        vm_manager.munmap(addr, size).expect("munmap should always succeed");
    }
}
