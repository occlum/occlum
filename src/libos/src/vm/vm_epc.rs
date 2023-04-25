// This file contains EPC related APIs and definitions.

use super::*;
use modular_bitfield::{
    bitfield,
    specifiers::{B1, B13, B16},
};
use sgx_trts::enclave::rsgx_is_supported_EDMM;
use std::ptr;

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
    WithoutEDMM,
    Simulation,
}

#[derive(Clone)]
pub enum EPC {
    ReservedMem(ReservedMem),
    UserRegionMem(UserRegionMem),
    GapMem,
}

impl SGXPlatform {
    pub fn new() -> Self {
        if rsgx_is_supported_EDMM() {
            SGXPlatform::WithEDMM
        } else {
            // SGXPlatform::WithoutEDMM
            SGXPlatform::Simulation
        }
    }

    pub fn alloc_user_space(
        &self,
        init_size: usize,
        max_size: usize,
    ) -> Result<(VMRange, VMRange)> {
        match self {
            SGXPlatform::WithEDMM => {
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
                    "allocated user space range is {:?}, gap range is {:?}",
                    total_user_space_range, gap_range
                );

                Ok((total_user_space_range, gap_range))
            }
            SGXPlatform::WithoutEDMM | SGXPlatform::Simulation => {
                // Without EDMM support, we only use reserved memory
                const magic_size: usize = 4 << 20; // Set gap size to make the memory layout same as platforms with EDMM.

                let block_a_size = init_size;
                let block_b_size = max_size - init_size;
                let block_a_start_addr = ReservedMem::alloc(block_a_size)?;

                let block_b_desired_start_addr = block_a_start_addr + block_a_size + magic_size;
                let block_b_start_addr =
                    ReservedMem::alloc_with_addr(block_b_desired_start_addr, block_b_size)?;
                let total_user_space_range =
                    VMRange::new(block_a_start_addr, block_b_start_addr + block_b_size)?;
                let gap_range =
                    VMRange::new(block_a_start_addr + block_a_size, block_b_start_addr)?;
                assert!(total_user_space_range.is_superset_of(&gap_range));

                Ok((total_user_space_range, gap_range))
            }
        }
    }

    pub fn free_user_space(&self, user_space_range: &VMRange, gap_range: &VMRange) {
        let user_space_ranges = user_space_range.subtract(gap_range);
        debug_assert!(user_space_ranges.len() == 2);
        match self {
            SGXPlatform::WithEDMM => {
                let reserved_mem = user_space_ranges[0];
                let user_region_mem = user_space_ranges[1];
                ReservedMem::free(reserved_mem.start(), reserved_mem.size());
                UserRegionMem::free(user_region_mem.start(), user_region_mem.size());
            }
            SGXPlatform::WithoutEDMM | SGXPlatform::Simulation => {
                user_space_ranges
                    .iter()
                    .for_each(|range| ReservedMem::free(range.start(), range.size()));
            }
        }
    }
}

#[derive(Clone)]
pub struct ReservedMem;

impl ReservedMem {
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

    fn free(addr: usize, size: usize) {
        let ret = unsafe { sgx_free_rsrv_mem(addr as *const c_void, size) };
        assert!(ret == 0);
    }

    pub fn modify_protection(&self, addr: usize, length: usize, protection: VMPerms) -> Result<()> {
        let ret = if rsgx_is_supported_EDMM() {
            unsafe {
                sgx_tprotect_rsrv_mem(addr as *const c_void, length, protection.bits() as i32)
            }
        } else {
            // For platforms without EDMM, sgx_tprotect_rsrv_mem is actually useless.
            // However, at least we can set pages to desired protections in the host kernel page table.
            let mut ret_val = 0;
            unsafe {
                occlum_ocall_mprotect(
                    &mut ret_val as *mut i32,
                    addr as *const c_void,
                    length,
                    protection.bits() as i32,
                )
            }
        };

        if ret != sgx_status_t::SGX_SUCCESS {
            return_errno!(ENOMEM, "reserved memory modify protection failure");
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct UserRegionMem;

impl UserRegionMem {
    fn alloc(size: usize) -> Result<usize> {
        let mut ptr = ptr::null_mut();
        let ret = unsafe {
            sgx_mm_alloc(
                ptr::null_mut(),
                size,
                EDMM_MAP_FLAGS::COMMIT_ON_DEMAND.bits() as i32,
                enclave_page_fault_handler_dummy,
                std::ptr::null_mut(),
                &mut ptr,
            )
        };
        if ptr.is_null() {
            return_errno!(ENOMEM, "run out of user region memory");
        }

        Ok(ptr as usize)
    }

    fn free(addr: usize, size: usize) {
        let ret = unsafe { sgx_mm_dealloc(addr as *mut c_void, size) };
        assert!(ret == 0);
    }

    pub fn modify_protection(&self, addr: usize, length: usize, protection: VMPerms) -> Result<()> {
        trace!(
            "user region modify protection, protection = {:?}, range = {:?}",
            protection,
            VMRange::new_with_size(addr, length).unwrap()
        );
        let ret = unsafe {
            sgx_mm_modify_permissions(addr as *mut c_void, length, protection.bits() as i32)
        };
        assert!(ret == 0);
        Ok(())
    }
}

impl Debug for EPC {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let output_str = match self {
            EPC::ReservedMem(_) => "reserved memory region",
            EPC::UserRegionMem(_) => "user region memory",
            EPC::GapMem => "Gap memory",
        };
        write!(f, "{}", output_str)
    }
}

impl EPC {
    pub fn new(range: &VMRange) -> Self {
        if rsgx_is_supported_EDMM() {
            let gap_range = USER_SPACE_VM_MANAGER
                .gap_range()
                .expect("gap range must exist");
            debug_assert!(!gap_range.is_superset_of(range));
            if range.end() <= gap_range.start() {
                EPC::ReservedMem(ReservedMem)
            } else {
                debug_assert!(gap_range.end() <= range.start());
                EPC::UserRegionMem(UserRegionMem)
            }
        } else {
            // Only reserved memory
            EPC::ReservedMem(ReservedMem)
        }
    }

    pub fn modify_protection(&self, addr: usize, length: usize, protection: VMPerms) -> Result<()> {
        // PT_GROWSDOWN should only be applied to stack segment or a segment mapped with the MAP_GROWSDOWN flag set.
        // Since the memory are managed by our own, mprotect ocall shouldn't use this flag. Otherwise, EINVAL will be thrown.
        let mut prot = protection.clone();
        prot.remove(VMPerms::GROWSDOWN);

        if rsgx_is_supported_EDMM() {
            match self {
                EPC::ReservedMem(mem) => mem.modify_protection(addr, length, protection),
                EPC::UserRegionMem(mem) => mem.modify_protection(addr, length, protection),
                EPC::GapMem => unreachable!(),
            }
        } else if let EPC::ReservedMem(mem) = self {
            mem.modify_protection(addr, length, protection)
        } else {
            unreachable!()
        }
    }
}

pub fn commit_epc_for_user_space(start_addr: usize, size: usize) -> Result<()> {
    info!(
        "commit epc: {:?}",
        VMRange::new_with_size(start_addr, size).unwrap()
    );
    let ret = unsafe { sgx_mm_commit(start_addr as *mut c_void, size) };
    assert!(ret == 0);
    if ret != 0 {
        return_errno!(Errno::from(ret as u32), "commit memory failure");
    } else {
        Ok(())
    }
}

#[repr(C)]
pub struct sgx_pfinfo_local {
    maddr: u64, // address for #PF.
    pfec: pfec,
    reserved: u32,
}

#[repr(C)]
union pfec {
    errcd: u32,
    inner: _pfec,
}

#[derive(Clone, Copy)]
#[bitfield]
struct _pfec {
    p: B1,
    rw: B1,
    reserved_01: B13,
    sgx: B1,
    reserved_02: B16,
}

bitflags! {
    pub struct EDMM_MAP_FLAGS : u32 {
        const RESERVE          = 0x1;
        const COMMIT_NOW       = 0x2;
        const COMMIT_ON_DEMAND = 0x4;
    }
}

#[repr(i32)]
enum PFHandlerRet {
    SGX_MM_EXCEPTION_CONTINUE_EXECUTION = -1,
    SGX_MM_EXCEPTION_CONTINUE_SEARCH = 0,
}

// This is a dummy function for sgx_mm_alloc. The real handler is "enclave_page_fault_handler" shown above.
#[no_mangle]
extern "C" fn enclave_page_fault_handler_dummy(
    sgx_pf_info: *const sgx_pfinfo,
    private_data: *mut c_void,
) -> i32 {
    // Don't do anything here. Modification of registers can cause the PF handling error.
    return PFHandlerRet::SGX_MM_EXCEPTION_CONTINUE_SEARCH as i32;
}

pub fn enclave_page_fault_handler(exception_info: sgx_misc_exinfo_t) -> Result<()> {
    // Safety: sgx_pfinfo and sgx_misc_exinfo_t have the same memory layout.
    let pf_info: sgx_pfinfo_local = unsafe { std::mem::transmute(exception_info) };
    let pf_addr = pf_info.maddr as usize;
    trace!("enclave page fault caught, pf_addr = 0x{:x}", pf_addr);

    // TODO: Maybe we can find a better way to know whether the page fault is due to protection violation or not.

    USER_SPACE_VM_MANAGER.handle_page_fault(pf_addr, false)?;

    Ok(())
}

#[allow(non_camel_case_types)]
pub type enclave_pf_handler_t =
    extern "C" fn(sgx_pf_info: *mut sgx_pfinfo, private_data: *const c_void) -> i32;

extern "C" {
    fn occlum_ocall_mprotect(
        retval: *mut i32,
        addr: *const c_void,
        len: usize,
        prot: i32,
    ) -> sgx_status_t;
}
