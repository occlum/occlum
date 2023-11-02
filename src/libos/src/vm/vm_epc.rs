// This file contains EPC related APIs and definitions.
use super::vm_util::AlignedZeroPage;
use super::*;
use edmm_api::EDMMLocalApi;
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

lazy_static! {
    static ref ZERO_PAGE: Vec<u8> = AlignedZeroPage::new_page_aligned_vec(PAGE_SIZE);
}

pub trait EPCAllocator {
    fn alloc(&self, size: usize) -> Result<usize> {
        return_errno!(ENOSYS, "operation not supported");
    }

    fn alloc_with_addr(&self, addr: usize, size: usize) -> Result<usize> {
        return_errno!(ENOSYS, "operation not supported");
    }

    fn free(&self, addr: usize, size: usize) -> Result<()> {
        return_errno!(ENOSYS, "operation not supported");
    }

    fn modify_permission(
        &self,
        addr: usize,
        length: usize,
        curr_perms: VMPerms,
        new_perms: VMPerms,
    ) -> Result<()> {
        return_errno!(ENOSYS, "operation not supported");
    }

    fn commit_memory(&self, start_addr: usize, size: usize, perms: Option<VMPerms>) -> Result<()> {
        return_errno!(ENOSYS, "operation not supported");
    }

    fn commit_memory_with_data(
        &self,
        start_addr: usize,
        data: &[u8],
        perms: VMPerms,
    ) -> Result<()> {
        return_errno!(ENOSYS, "operation not supported");
    }
}

impl EPCAllocator for ReservedMem {
    fn alloc(&self, size: usize) -> Result<usize> {
        let ptr = unsafe { sgx_alloc_rsrv_mem(size) };
        if ptr.is_null() {
            return_errno!(ENOMEM, "run out of reserved memory");
        }
        Ok(ptr as usize)
    }

    fn alloc_with_addr(&self, addr: usize, size: usize) -> Result<usize> {
        let ptr = unsafe { sgx_alloc_rsrv_mem_ex(addr as *const c_void, size) };
        if ptr.is_null() {
            return_errno!(ENOMEM, "can't allocate reserved memory at desired address");
        }
        Ok(ptr as usize)
    }

    fn free(&self, addr: usize, size: usize) -> Result<()> {
        let ret = unsafe { sgx_free_rsrv_mem(addr as *const c_void, size) };
        assert!(ret == 0);
        Ok(())
    }

    fn modify_permission(
        &self,
        addr: usize,
        length: usize,
        curr_perms: VMPerms,
        new_perms: VMPerms,
    ) -> Result<()> {
        // PT_GROWSDOWN should only be applied to stack segment or a segment mapped with the MAP_GROWSDOWN flag set.
        // Since the memory are managed by our own, mprotect ocall shouldn't use this flag. Otherwise, EINVAL will be thrown.
        let mut new_perms = new_perms.clone();
        new_perms.remove(VMPerms::GROWSDOWN);
        let mut ret_val = 0;
        let ret = if rsgx_is_supported_EDMM() {
            // Use raw implementation to reduce SDK's overhead
            trace!(
                "raw modify_permission curr_perms: {:?}, new_perms: {:?}",
                curr_perms,
                new_perms
            );
            EDMMLocalApi::modify_permissions(addr, length, curr_perms, new_perms).unwrap();
            sgx_status_t::SGX_SUCCESS
        } else {
            // For platforms without EDMM, sgx_tprotect_rsrv_mem is actually useless.
            // However, at least we can set pages to desired protections in the host kernel page table.
            extern "C" {
                fn occlum_ocall_mprotect(
                    retval: *mut i32,
                    addr: *const c_void,
                    len: usize,
                    prot: i32,
                ) -> sgx_status_t;
            }
            unsafe {
                occlum_ocall_mprotect(
                    &mut ret_val as *mut i32,
                    addr as *const c_void,
                    length,
                    new_perms.bits() as i32,
                )
            }
        };

        if ret != sgx_status_t::SGX_SUCCESS || ret_val != 0 {
            error!("ocall ret = {:?}, ret_val = {:?}", ret, ret_val);
            return_errno!(ENOMEM, "reserved memory modify protection failure");
        }

        Ok(())
    }
}

impl EPCAllocator for UserRegionMem {
    fn alloc(&self, size: usize) -> Result<usize> {
        let alloc_options = AllocOptions::new()
            .set_flags(AllocFlags::COMMIT_ON_DEMAND)
            .set_handler(enclave_page_fault_handler_dummy, 0);
        let ptr = unsafe { EmmAlloc.alloc(AllocAddr::Any, size, alloc_options) }
            .map_err(|e| errno!(Errno::from(e as u32)))?;

        Ok(ptr.addr().get())
    }

    fn free(&self, addr: usize, size: usize) -> Result<()> {
        let ptr = NonNull::<u8>::new(addr as *mut u8).unwrap();
        unsafe { EmmAlloc.dealloc(ptr, size) }.map_err(|e| errno!(Errno::from(e as u32)))?;
        Ok(())
    }

    fn modify_permission(
        &self,
        addr: usize,
        length: usize,
        curr_perms: VMPerms,
        new_perms: VMPerms,
    ) -> Result<()> {
        // PT_GROWSDOWN should only be applied to stack segment or a segment mapped with the MAP_GROWSDOWN flag set.
        // Since the memory are managed by our own, mprotect ocall shouldn't use this flag. Otherwise, EINVAL will be thrown.
        let mut new_perms = new_perms.clone();
        new_perms.remove(VMPerms::GROWSDOWN);

        trace!(
            "user region modify protection, protection = {:?}, range = {:?}",
            new_perms,
            VMRange::new_with_size(addr, length).unwrap()
        );

        EDMMLocalApi::modify_permissions(addr, length, curr_perms, new_perms)?;

        Ok(())
    }

    fn commit_memory(&self, start_addr: usize, size: usize, perms: Option<VMPerms>) -> Result<()> {
        match perms {
            Some(perms) if perms != VMPerms::DEFAULT => {
                if size == PAGE_SIZE {
                    EDMMLocalApi::commit_with_data(start_addr, ZERO_PAGE.as_slice(), perms)?;
                } else {
                    let data = AlignedZeroPage::new_page_aligned_vec(size);
                    EDMMLocalApi::commit_with_data(start_addr, data.as_slice(), perms)?;
                }
            }
            _ => EDMMLocalApi::commit_memory(start_addr, size)?,
        }

        Ok(())
    }

    fn commit_memory_with_data(
        &self,
        start_addr: usize,
        data: &[u8],
        new_perms: VMPerms,
    ) -> Result<()> {
        EDMMLocalApi::commit_with_data(start_addr, data, new_perms)
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
        if matches!(self, SGXPlatform::WithEDMM) && max_size > init_size {
            let user_region_size = max_size - init_size;

            let reserved_mem_start_addr = ReservedMem.alloc(init_size)?;

            let user_region_start_addr = UserRegionMem.alloc(user_region_size)?;

            let total_user_space_range = VMRange::new(
                reserved_mem_start_addr,
                user_region_start_addr + user_region_size,
            )?;
            let gap_range =
                VMRange::new(reserved_mem_start_addr + init_size, user_region_start_addr)?;

            debug!(
                "allocated user space range is {:?}, gap range is {:?}. reserved_mem range is {:?}, user region range is {:?}",
                total_user_space_range, gap_range, VMRange::new_with_size(reserved_mem_start_addr, init_size),
                VMRange::new_with_size(user_region_start_addr, user_region_size)
            );

            Ok((total_user_space_range, Some(gap_range)))
        } else {
            // For platform with no-edmm support, or the max_size is the same as init_size, use reserved memory for the whole userspace
            let reserved_mem_start_addr = ReservedMem.alloc(max_size)?;
            let total_user_space_range =
                VMRange::new(reserved_mem_start_addr, reserved_mem_start_addr + max_size)?;

            debug!(
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
            ReservedMem
                .free(reserved_mem.start(), reserved_mem.size())
                .unwrap();
            UserRegionMem
                .free(user_region_mem.start(), user_region_mem.size())
                .unwrap();
        } else {
            // For platforms with EDMM but max_size equals init_size or the paltforms without EDMM, there is no gap range.
            debug_assert!(user_space_ranges.len() == 1);
            let reserved_mem = user_space_ranges[0];
            ReservedMem
                .free(reserved_mem.start(), reserved_mem.size())
                .unwrap();
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

    pub fn epc_allocator(&self) -> &dyn EPCAllocator {
        match self {
            EPCMemType::Reserved => &ReservedMem,
            EPCMemType::UserRegion => &UserRegionMem,
        }
    }
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
    debug!(
        "enclave page fault caught, pf_addr = 0x{:x}, error code = {:?}",
        pf_addr, pf_errcd
    );

    USER_SPACE_VM_MANAGER.handle_page_fault(rip, pf_addr, pf_errcd, kernel_triggers)?;

    Ok(())
}

mod edmm_api {
    use super::*;
    use std::marker::PhantomData;
    use std::mem;

    pub(super) struct EDMMLocalApi;

    impl EDMMLocalApi {
        // To replace sgx_mm_commit
        pub(super) fn commit_memory(start_addr: usize, size: usize) -> Result<()> {
            let si = sec_info_t::new_for_commit_memory();
            for page in (start_addr..start_addr + size).step_by(PAGE_SIZE) {
                let ret = unsafe { do_eaccept(&si as *const sec_info_t, page) };
                if ret != 0 {
                    return_errno!(EFAULT, "do_eaccept failure");
                }
            }
            Ok(())
        }

        // To replace sgx_mm_commit_data
        pub(super) fn commit_with_data(addr: usize, data: &[u8], perm: VMPerms) -> Result<()> {
            let si = sec_info_t::new_for_commit_with_data(&perm);
            let size = data.len();
            let mut src_raw_ptr = data.as_ptr() as usize;
            for dest_page in (addr..addr + size).step_by(PAGE_SIZE) {
                let ret =
                    unsafe { do_eacceptcopy(&si as *const sec_info_t, dest_page, src_raw_ptr) };
                if ret != 0 {
                    return_errno!(EFAULT, "do_eacceptcopy failure");
                }
                src_raw_ptr += PAGE_SIZE;
            }

            Self::modify_permissions(addr, size, VMPerms::DEFAULT, perm)?;

            Ok(())
        }

        // To replace sgx_mm_modify_permissions
        pub(super) fn modify_permissions(
            addr: usize,
            length: usize,
            curr_perms: VMPerms,
            new_perms: VMPerms,
        ) -> Result<()> {
            if curr_perms == new_perms {
                return Ok(());
            }

            let flags_from = curr_perms.bits() | SGX_EMA_PAGE_TYPE_REG;
            let flags_to = new_perms.bits() | SGX_EMA_PAGE_TYPE_REG;
            let ret =
                unsafe { sgx_mm_modify_ocall(addr, length, flags_from as i32, flags_to as i32) };
            if ret != 0 {
                return_errno!(EFAULT, "sgx_mm_modify_ocall failure");
            }

            let si = sec_info_t::new_for_modify_permission(&new_perms);
            for page in (addr..addr + length).step_by(PAGE_SIZE) {
                debug_assert!(page % PAGE_SIZE == 0);

                if new_perms.bits() | curr_perms.bits() != curr_perms.bits() {
                    unsafe { do_emodpe(&si as *const sec_info_t, page) };
                    // Check this return value is useless. RAX is set to SE_EMODPE which is 6 defined in SDK.
                }
                // If new permission is RWX, no EMODPR needed in untrusted part, hence no EACCEPT
                if new_perms != VMPerms::ALL {
                    let ret = unsafe { do_eaccept(&si, page) };
                    if ret != 0 {
                        return_errno!(EFAULT, "do_eaccept failure");
                    }
                }
            }

            if new_perms == VMPerms::NONE {
                let ret = unsafe {
                    sgx_mm_modify_ocall(
                        addr,
                        length,
                        (SGX_EMA_PAGE_TYPE_REG | SGX_EMA_PROT_NONE) as i32,
                        (SGX_EMA_PAGE_TYPE_REG | SGX_EMA_PROT_NONE) as i32,
                    )
                };
                if ret != 0 {
                    return_errno!(EFAULT, "sgx_mm_modify_ocall failure for permission None");
                }
            }

            Ok(())
        }
    }

    extern "C" {
        // EACCEPT
        fn do_eaccept(si: *const sec_info_t, addr: usize) -> i32;

        // EMODPE
        fn do_emodpe(si: *const sec_info_t, addr: usize) -> i32;

        // EACCEPTCOPY
        fn do_eacceptcopy(si: *const sec_info_t, dest: usize, src: usize) -> i32;
    }

    #[allow(non_camel_case_types)]
    #[repr(C, align(512))]
    struct sec_info_t {
        flags: u64,
        reserved: [u64; 7],
    }

    impl sec_info_t {
        const SGX_EMA_STATE_PENDING: u64 = 0x08; // pending state
        const SGX_EMA_STATE_PR: u64 = 0x20; // permission restriction state

        fn new_for_modify_permission(new_perms: &VMPerms) -> Self {
            Self {
                flags: ((new_perms.bits() | SGX_EMA_PAGE_TYPE_REG) as u64) | Self::SGX_EMA_STATE_PR,
                reserved: [0; 7],
            }
        }

        fn new_for_commit_memory() -> Self {
            Self {
                flags: ((VMPerms::DEFAULT.bits() | SGX_EMA_PAGE_TYPE_REG) as u64)
                    | Self::SGX_EMA_STATE_PENDING,
                reserved: [0; 7],
            }
        }

        fn new_for_commit_with_data(protection: &VMPerms) -> Self {
            Self {
                flags: (protection.bits() | SGX_EMA_PAGE_TYPE_REG) as u64,
                reserved: [0; 7],
            }
        }
    }

    macro_rules! weak {
        (fn $name:ident($($t:ty),*) -> $ret:ty) => (
            let ref $name: ExternWeak<unsafe extern "C" fn($($t),*) -> $ret> = {
                extern "C" {
                    #[linkage = "extern_weak"]
                    static $name: *const c_void;
                }
                #[allow(unused_unsafe)]
                ExternWeak::new(unsafe { $name })
            };
        )
    }

    pub struct ExternWeak<F> {
        weak_ptr: *const c_void,
        _marker: PhantomData<F>,
    }

    impl<F> ExternWeak<F> {
        #[inline]
        pub fn new(weak_ptr: *const c_void) -> Self {
            ExternWeak {
                weak_ptr,
                _marker: PhantomData,
            }
        }

        #[inline]
        pub fn get(&self) -> Option<F> {
            unsafe {
                if self.weak_ptr.is_null() {
                    None
                } else {
                    Some(mem::transmute_copy::<*const c_void, F>(&self.weak_ptr))
                }
            }
        }
    }

    macro_rules! edmm_api {
        (fn $name:ident($($arg_name:ident: $t:ty),*) -> $ret:ty) => (
            unsafe fn $name($($arg_name:$t),*) -> $ret {
                weak! { fn $name($($t),*) -> $ret }

                if let Some(fun) = $name.get() {
                    fun($($arg_name),*)
                } else {
                    (ENOSYS) as $ret
                }
            }
        )
    }

    // Special symbol which is not defined in sim mode
    edmm_api! {
        fn sgx_mm_modify_ocall(addr: usize, size: usize, flags_from: i32, flags_to: i32) -> i32
    }
}
