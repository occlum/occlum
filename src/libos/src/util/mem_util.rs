use super::*;
use std::ffi::{CStr, CString};
use std::mem::size_of;
use std::ptr;
use std::slice;
use vm::VMRange;

/// Memory utilities that deals with primitive types passed from user process
/// running inside enclave
pub mod from_user {
    use super::*;

    /// Check the user pointer is within the readable memory range of the user process
    pub fn check_ptr<T>(user_ptr: *const T) -> Result<()> {
        if !is_inside_user_space(user_ptr as *const u8, size_of::<T>()) {
            return_errno!(EFAULT, "pointer is not in the user space");
        }
        Ok(())
    }

    /// Check the mutable user pointer is within the writable memory of the user process
    pub fn check_mut_ptr<T>(user_ptr: *mut T) -> Result<()> {
        // The user space is both readable and writable on SGX1.
        // TODO: Fine-tune the checking on SGX2.
        check_ptr(user_ptr)
    }

    /// Check the readonly array is within the readable memory of the user process
    pub fn check_array<T>(user_buf: *const T, count: usize) -> Result<()> {
        let checked_len = count
            .checked_mul(size_of::<T>())
            .ok_or_else(|| errno!(EINVAL, "the array is too long"))?;
        if !is_inside_user_space(user_buf as *const u8, checked_len) {
            return_errno!(EFAULT, "the whole buffer is not in the user space");
        }
        Ok(())
    }

    /// Check the mutable array is within the writable memory of the user process
    pub fn check_mut_array<T>(user_buf: *mut T, count: usize) -> Result<()> {
        // The user space is both readable and writable on SGX1.
        // TODO: Fine-tune the checking on SGX2.
        check_array(user_buf, count)
    }

    pub fn make_slice<'a, T>(user_buf: *const T, count: usize) -> Result<&'a [T]> {
        check_array(user_buf, count)?;
        Ok(unsafe { slice::from_raw_parts(user_buf, count) })
    }

    pub fn make_mut_slice<'a, T>(user_buf: *mut T, count: usize) -> Result<&'a mut [T]> {
        check_mut_array(user_buf, count)?;
        Ok(unsafe { slice::from_raw_parts_mut(user_buf, count) })
    }

    pub fn make_ref<'a, T>(user_ptr: *const T) -> Result<&'a T> {
        check_ptr(user_ptr)?;
        Ok(unsafe { &*user_ptr })
    }

    pub fn make_mut_ref<'a, T>(user_ptr: *mut T) -> Result<&'a mut T> {
        check_mut_ptr(user_ptr)?;
        Ok(unsafe { &mut *user_ptr })
    }

    /// Clone a C-string from the user process safely
    pub fn clone_cstring_safely(out_ptr: *const c_char) -> Result<CString> {
        if out_ptr.is_null() {
            return_errno!(EINVAL, "NULL address is invalid");
        }

        // confirm that at least the first byte of the string is from user
        check_ptr(out_ptr)?;

        let cstr = unsafe { CStr::from_ptr(out_ptr) };
        if !is_inside_user_space(out_ptr as *const u8, cstr.to_bytes_with_nul().len()) {
            return_errno!(EFAULT, "the whole buffer is not in the user space");
        }

        let cstring = CString::from(cstr);
        Ok(cstring)
    }

    /// Clone a C-string array (const char*[]) from the user process safely
    ///
    /// This array must be ended with a NULL pointer.
    pub fn clone_cstrings_safely(user_ptr: *const *const c_char) -> Result<Vec<CString>> {
        let mut cstrings = Vec::new();
        if user_ptr == ptr::null() {
            return Ok(cstrings);
        }

        let mut user_ptr = user_ptr;
        loop {
            check_ptr(user_ptr)?;

            let cstr_ptr = {
                let cstr_ptr = unsafe { *user_ptr };
                if cstr_ptr == ptr::null() {
                    break;
                }
                cstr_ptr
            };
            let cstring = clone_cstring_safely(cstr_ptr)?;
            cstrings.push(cstring);

            user_ptr = unsafe { user_ptr.offset(1) };
        }
        Ok(cstrings)
    }

    /// Check if the provided buffer is within the current user space
    ///
    /// addr: the start address
    /// len: the length in byte
    fn is_inside_user_space(addr: *const u8, len: usize) -> bool {
        let current = current!();
        let user_range = current.vm().get_process_range();
        let ur_start = user_range.start();
        let ur_end = user_range.end();
        let addr_start = addr as usize;
        addr_start >= ur_start && addr_start < ur_end && ur_end - addr_start >= len
    }
}

/// Memory utilities that deals with primitive types passed from outside the enclave
pub mod from_untrusted {
    use super::*;

    /// Check the untrusted pointer is outside the enclave
    pub fn check_ptr<T>(out_ptr: *const T) -> Result<()> {
        if !sgx_trts::trts::rsgx_raw_is_outside_enclave(out_ptr as *const u8, size_of::<T>()) {
            return_errno!(EFAULT, "the pointer is not outside enclave");
        }
        Ok(())
    }

    /// Check the untrusted array is outside the enclave
    pub fn check_array<T>(out_ptr: *const T, count: usize) -> Result<()> {
        let checked_len = count
            .checked_mul(size_of::<T>())
            .ok_or_else(|| errno!(EINVAL, "the array is too long"))?;
        if !sgx_trts::trts::rsgx_raw_is_outside_enclave(out_ptr as *const u8, checked_len) {
            return_errno!(EFAULT, "the whole buffer is not outside enclave");
        }
        Ok(())
    }

    /// Clone a C-string from outside the enclave
    pub fn clone_cstring_safely(out_ptr: *const c_char) -> Result<CString> {
        if out_ptr.is_null() {
            return_errno!(EINVAL, "NULL address is invalid");
        }

        // confirm that at least the first byte of the string is out side of enclave
        check_ptr(out_ptr)?;

        let cstr = unsafe { CStr::from_ptr(out_ptr) };
        if !sgx_trts::trts::rsgx_raw_is_outside_enclave(
            out_ptr as *const u8,
            cstr.to_bytes_with_nul().len(),
        ) {
            return_errno!(EFAULT, "the string is not outside enclave");
        }

        let cstring = CString::from(cstr);
        Ok(cstring)
    }

    /// Clone a C-string array (const char*[]) from outside the enclave
    ///
    /// This array must be ended with a NULL pointer.
    pub fn clone_cstrings_safely(out_ptr: *const *const c_char) -> Result<Vec<CString>> {
        let mut cstrings = Vec::new();
        if out_ptr == ptr::null() {
            return Ok(cstrings);
        }

        let mut out_ptr = out_ptr;
        loop {
            check_ptr(out_ptr)?;

            let cstr_ptr = {
                let cstr_ptr = unsafe { *out_ptr };
                if cstr_ptr == ptr::null() {
                    break;
                }
                cstr_ptr
            };
            let cstring = clone_cstring_safely(cstr_ptr)?;
            cstrings.push(cstring);

            out_ptr = unsafe { out_ptr.offset(1) };
        }
        Ok(cstrings)
    }
}
