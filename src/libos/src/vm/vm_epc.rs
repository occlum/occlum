// This file contains EPC related APIs and definitions.
use super::*;
use sgx_trts::emm::{
    AllocAddr, AllocFlags, AllocOptions, EmmAlloc, HandleResult, PageFaultHandler, Perm,
};
use sgx_trts::enclave::rsgx_is_supported_EDMM;
use std::ptr::NonNull;

// Memory Layout for Platforms with EDMM support
//
// Addr low -> high
// |---------------------------------------------||---------------------||--------------------------------------|
//     Reserved Memory                                Gap Range                User Region Memory
//    (commit memory when loading the enclave)       (used by SDK)           (commit on demand when PF occurs)
//
// For platforms without EDMM support, we only use reserved memory.

pub enum SGXPlatform {
    WithEDMM,
    NoEDMM,
}

#[derive(Clone)]
pub enum EPCMemType {
    Reserved,
    UserRegion,
}

pub struct ReservedMem;
pub struct UserRegionMem;

#[repr(C, align(4096))]
#[derive(Clone)]
struct ZeroPage([u8; PAGE_SIZE]);

impl ZeroPage {
    fn new() -> Self {
        Self([0; PAGE_SIZE])
    }

    fn new_page_aligned_vec(size: usize) -> Vec<u8> {
        debug_assert!(size % PAGE_SIZE == 0);
        let page_num = size / PAGE_SIZE;
        let mut page_vec = vec![Self::new(); page_num];

        let ptr = page_vec.as_mut_ptr();

        let size = page_num * std::mem::size_of::<Self>();
        std::mem::forget(page_vec);

        unsafe { Vec::from_raw_parts(ptr as *mut u8, size, size) }
    }
}

lazy_static! {
    static ref ZERO_PAGE: Vec<u8> = ZeroPage::new_page_aligned_vec(PAGE_SIZE);
}

pub trait EPCAllocator {
    fn alloc(size: usize) -> Result<usize> {
        return_errno!(ENOSYS, "operation not supported");
    }

    fn alloc_with_addr(addr: usize, size: usize) -> Result<usize> {
        return_errno!(ENOSYS, "operation not supported");
    }

    fn free(addr: usize, size: usize) -> Result<()> {
        return_errno!(ENOSYS, "operation not supported");
    }

    fn modify_protection(addr: usize, length: usize, protection: VMPerms) -> Result<()> {
        return_errno!(ENOSYS, "operation not supported");
    }

    fn mem_type() -> EPCMemType;
}

impl EPCAllocator for ReservedMem {
    fn alloc(size: usize) -> Result<usize> {
        let ptr = unsafe { sgx_alloc_rsrv_mem(size) };
        if ptr.is_null() {
            return_errno!(ENOMEM, "run out of reserved memory");
        }
        Ok(ptr as usize)
    }

    fn alloc_with_addr(addr: usize, size: usize) -> Result<usize> {
        let ptr = unsafe { sgx_alloc_rsrv_mem_ex(addr as *const c_void, size) };
        if ptr.is_null() {
            return_errno!(ENOMEM, "can't allocate reserved memory at desired address");
        }
        Ok(ptr as usize)
    }

    fn free(addr: usize, size: usize) -> Result<()> {
        let ret = unsafe { sgx_free_rsrv_mem(addr as *const c_void, size) };
        assert!(ret == 0);
        Ok(())
    }

    fn modify_protection(addr: usize, length: usize, protection: VMPerms) -> Result<()> {
        let mut ret_val = 0;
        let ret = if rsgx_is_supported_EDMM() {
            unsafe {
                sgx_tprotect_rsrv_mem(addr as *const c_void, length, protection.bits() as i32)
            }
        } else {
            // For platforms without EDMM, sgx_tprotect_rsrv_mem is actually useless.
            // However, at least we can set pages to desired protections in the host kernel page table.
            unsafe {
                occlum_ocall_mprotect(
                    &mut ret_val as *mut i32,
                    addr as *const c_void,
                    length,
                    protection.bits() as i32,
                )
            }
        };

        if ret != sgx_status_t::SGX_SUCCESS || ret_val != 0 {
            return_errno!(ENOMEM, "reserved memory modify protection failure");
        }

        Ok(())
    }

    fn mem_type() -> EPCMemType {
        EPCMemType::Reserved
    }
}

impl EPCAllocator for UserRegionMem {
    fn alloc(size: usize) -> Result<usize> {
        let alloc_options = AllocOptions::new()
            .set_flags(AllocFlags::COMMIT_ON_DEMAND)
            .set_handler(enclave_page_fault_handler_dummy, 0);
        let ptr = unsafe { EmmAlloc.alloc(AllocAddr::Any, size, alloc_options) }
            .map_err(|e| errno!(Errno::from(e as u32)))?;

        Ok(ptr.addr().get())
    }

    fn free(addr: usize, size: usize) -> Result<()> {
        let ptr = NonNull::<u8>::new(addr as *mut u8).unwrap();
        unsafe { EmmAlloc.dealloc(ptr, size) }.map_err(|e| errno!(Errno::from(e as u32)))?;
        Ok(())
    }

    fn modify_protection(addr: usize, length: usize, protection: VMPerms) -> Result<()> {
        trace!(
            "user region modify protection, protection = {:?}, range = {:?}",
            protection,
            VMRange::new_with_size(addr, length).unwrap()
        );
        let ptr = NonNull::<u8>::new(addr as *mut u8).unwrap();
        unsafe {
            EmmAlloc.modify_permissions(ptr, length, Perm::from_bits(protection.bits()).unwrap())
        }
        .map_err(|e| errno!(Errno::from(e as u32)))?;

        Ok(())
    }

    fn mem_type() -> EPCMemType {
        EPCMemType::UserRegion
    }
}

impl UserRegionMem {
    fn commit_memory(start_addr: usize, size: usize) -> Result<()> {
        let ptr = NonNull::<u8>::new(start_addr as *mut u8).unwrap();
        unsafe { EmmAlloc.commit(ptr, size) }.map_err(|e| errno!(Errno::from(e as u32)))?;
        Ok(())
    }

    fn commit_memory_with_new_permission(
        start_addr: usize,
        size: usize,
        new_perms: VMPerms,
    ) -> Result<()> {
        let ptr = NonNull::<u8>::new(start_addr as *mut u8).unwrap();
        let perm = Perm::from_bits(new_perms.bits()).unwrap();
        if size == PAGE_SIZE {
            unsafe { EmmAlloc::commit_with_data(ptr, ZERO_PAGE.as_slice(), perm) }
                .map_err(|e| errno!(Errno::from(e as u32)))?;
        } else {
            let data = ZeroPage::new_page_aligned_vec(size);
            unsafe { EmmAlloc::commit_with_data(ptr, data.as_slice(), perm) }
                .map_err(|e| errno!(Errno::from(e as u32)))?;
        }
        Ok(())
    }

    fn commit_memory_and_init_with_file(
        start_addr: usize,
        size: usize,
        file: &FileRef,
        file_offset: usize,
        new_perms: VMPerms,
    ) -> Result<()> {
        let mut data = ZeroPage::new_page_aligned_vec(size);
        let len = file
            .read_at(file_offset, data.as_mut_slice())
            .map_err(|_| errno!(EACCES, "failed to init memory from file"))?;

        let ptr = NonNull::<u8>::new(start_addr as *mut u8).unwrap();
        let perm = Perm::from_bits(new_perms.bits()).unwrap();

        unsafe { EmmAlloc::commit_with_data(ptr, data.as_slice(), perm) }
            .map_err(|e| errno!(Errno::from(e as u32)))?;
        Ok(())
    }
}

impl SGXPlatform {
    pub fn new() -> Self {
        if rsgx_is_supported_EDMM() {
            SGXPlatform::WithEDMM
        } else {
            SGXPlatform::NoEDMM // including SGX simulation mode
        }
    }

    pub fn alloc_user_space(
        &self,
        init_size: usize,
        max_size: usize,
    ) -> Result<(VMRange, Option<VMRange>)> {
        debug!(
            "alloc user space init size = {:?}, max size = {:?}",
            init_size, max_size
        );
        if matches!(self, SGXPlatform::WithEDMM) && max_size > init_size {
            let user_region_size = max_size - init_size;

            let reserved_mem_start_addr = ReservedMem::alloc(init_size)?;

            let user_region_start_addr = UserRegionMem::alloc(user_region_size)?;

            let total_user_space_range = VMRange::new(
                reserved_mem_start_addr,
                user_region_start_addr + user_region_size,
            )?;
            let gap_range =
                VMRange::new(reserved_mem_start_addr + init_size, user_region_start_addr)?;

            info!(
                "allocated user space range is {:?}, gap range is {:?}. reserved_mem range is {:?}, user region range is {:?}",
                total_user_space_range, gap_range, VMRange::new_with_size(reserved_mem_start_addr, init_size),
                VMRange::new_with_size(user_region_start_addr, user_region_size)
            );

            Ok((total_user_space_range, Some(gap_range)))
        } else {
            // For platform with no-edmm support, or the max_size is the same as init_size, use reserved memory for the whole userspace
            let reserved_mem_start_addr = ReservedMem::alloc(max_size)?;
            let total_user_space_range =
                VMRange::new(reserved_mem_start_addr, reserved_mem_start_addr + max_size)?;

            info!(
                "allocated user space range is {:?}, gap range is None",
                total_user_space_range
            );

            Ok((total_user_space_range, None))
        }
    }

    pub fn free_user_space(&self, user_space_range: &VMRange, gap_range: Option<&VMRange>) {
        let user_space_ranges = if let Some(gap_range) = gap_range {
            user_space_range.subtract(gap_range)
        } else {
            vec![*user_space_range]
        };

        if user_space_ranges.len() == 2 {
            debug_assert!(matches!(self, SGXPlatform::WithEDMM));
            let reserved_mem = user_space_ranges[0];
            let user_region_mem = user_space_ranges[1];
            ReservedMem::free(reserved_mem.start(), reserved_mem.size()).unwrap();
            UserRegionMem::free(user_region_mem.start(), user_region_mem.size()).unwrap();
        } else {
            // For platforms with EDMM but max_size equals init_size or the paltforms without EDMM, there is no gap range.
            debug_assert!(user_space_ranges.len() == 1);
            let reserved_mem = user_space_ranges[0];
            ReservedMem::free(reserved_mem.start(), reserved_mem.size()).unwrap();
        }
    }
}

impl Debug for EPCMemType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let output_str = match self {
            EPCMemType::Reserved => "reserved memory region",
            EPCMemType::UserRegion => "user region memory",
        };
        write!(f, "{}", output_str)
    }
}

impl EPCMemType {
    pub fn new(range: &VMRange) -> Self {
        trace!("EPC new range = {:?}", range);
        if rsgx_is_supported_EDMM() {
            if let Some(gap_range) = USER_SPACE_VM_MANAGER.gap_range() {
                debug_assert!({
                    if range.size() > 0 {
                        !gap_range.overlap_with(range)
                    } else {
                        // Ignore for sentry VMA
                        true
                    }
                });
                if range.end() <= gap_range.start() {
                    EPCMemType::Reserved
                } else {
                    debug_assert!(gap_range.end() <= range.start());
                    EPCMemType::UserRegion
                }
            } else {
                // There is no gap, this indicates that there is no user region memory
                EPCMemType::Reserved
            }
        } else {
            // Only reserved memory
            EPCMemType::Reserved
        }
    }

    pub fn modify_protection(&self, addr: usize, length: usize, protection: VMPerms) -> Result<()> {
        // PT_GROWSDOWN should only be applied to stack segment or a segment mapped with the MAP_GROWSDOWN flag set.
        // Since the memory are managed by our own, mprotect ocall shouldn't use this flag. Otherwise, EINVAL will be thrown.
        let mut prot = protection.clone();
        prot.remove(VMPerms::GROWSDOWN);

        match self {
            EPCMemType::Reserved => ReservedMem::modify_protection(addr, length, prot),
            EPCMemType::UserRegion => UserRegionMem::modify_protection(addr, length, prot),
        }
    }
}

pub fn commit_memory(start_addr: usize, size: usize, new_perms: Option<VMPerms>) -> Result<()> {
    trace!(
        "commit epc: {:?}, new permission: {:?}",
        VMRange::new_with_size(start_addr, size).unwrap(),
        new_perms
    );

    // We should make memory commit and permission change atomic to prevent data races. Thus, if the new perms
    // are not the default permission (RW), we implement a different function by calling EACCEPTCOPY
    match new_perms {
        Some(perms) if perms != VMPerms::DEFAULT => {
            UserRegionMem::commit_memory_with_new_permission(start_addr, size, perms)
        }
        _ => UserRegionMem::commit_memory(start_addr, size),
    }
}

pub fn commit_memory_and_init_with_file(
    start_addr: usize,
    size: usize,
    file: &FileRef,
    file_offset: usize,
    new_perms: VMPerms,
) -> Result<()> {
    UserRegionMem::commit_memory_and_init_with_file(start_addr, size, file, file_offset, new_perms)
}

// This is a dummy function for sgx_mm_alloc. The real handler is "enclave_page_fault_handler" shown below.
extern "C" fn enclave_page_fault_handler_dummy(
    pfinfo: &sgx_pfinfo,
    private: usize,
) -> HandleResult {
    // Don't do anything here. Modification of registers can cause the PF handling error.
    return HandleResult::Search;
}

pub fn enclave_page_fault_handler(
    rip: usize,
    exception_info: sgx_misc_exinfo_t,
    kernel_triggers: bool,
) -> Result<()> {
    let pf_addr = exception_info.faulting_address as usize;
    let pf_errcd = exception_info.error_code;
    trace!(
        "enclave page fault caught, pf_addr = 0x{:x}, error code = {:?}",
        pf_addr,
        pf_errcd
    );

    USER_SPACE_VM_MANAGER.handle_page_fault(rip, pf_addr, pf_errcd, kernel_triggers)?;

    Ok(())
}

extern "C" {
    fn occlum_ocall_mprotect(
        retval: *mut i32,
        addr: *const c_void,
        len: usize,
        prot: i32,
    ) -> sgx_status_t;
}
