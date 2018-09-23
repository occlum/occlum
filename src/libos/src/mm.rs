use sgx_types::{c_void, c_int, size_t};
use sgx_trts::libc;
use std::mem;
use std::marker::Send;
use std::marker::Sync;

#[derive(Clone, Debug)]
pub struct MemObj {
    mem_ptr: *mut c_void,
    mem_size: usize,
    mem_align: usize,
}

impl MemObj {
    pub fn new(mem_size: usize, mem_align: usize)
        -> Result<Self, &'static str>
    {
        if mem_size == 0 || !is_power_of_two(mem_align) ||
            mem_align % mem::size_of::<*const c_void>() != 0 {
            return Err("Invalid argument");
        }

        let mem_ptr = unsafe { aligned_malloc(mem_size, mem_align) };
        if mem_ptr == (0 as *mut c_void) {
            return Err("Out of memory");
        };
        unsafe { memset(mem_ptr, 0 as c_int, mem_size as size_t) };

        Ok(MemObj {
            mem_ptr,
            mem_size,
            mem_align,
        })
    }

    pub fn get_addr(&self) -> usize {
        self.mem_ptr as usize
    }
}

impl Default for MemObj {
    fn default() -> Self {
        MemObj {
            mem_ptr: 0 as *mut c_void,
            mem_size: 0,
            mem_align: 1
        }
    }
}

impl Drop for MemObj {
    fn drop(&mut self) {
        if self.mem_ptr != (0 as *mut c_void) {
            unsafe { free(self.mem_ptr); }
        }
    }
}

unsafe impl Send for MemObj {}
unsafe impl Sync for MemObj {}


fn is_power_of_two(x: usize) -> bool {
    return (x != 0) && ((x & (x - 1)) == 0);
}

unsafe fn aligned_malloc(mem_size: usize, mem_align: usize) -> *mut c_void {
    let mut mem_ptr = ::core::ptr::null_mut();
    let ret = libc::posix_memalign(&mut mem_ptr, mem_align, mem_size);
    if ret == 0 {
        mem_ptr
    } else {
        0 as *mut c_void
    }
}

unsafe fn free(mem_ptr: *mut c_void) {
    libc::free(mem_ptr)
}

#[link(name = "sgx_tstdc")]
extern {
    pub fn memset(p: *mut c_void, c: c_int, n: size_t) -> *mut c_void;
}
