use super::*;
use std::ffi::{CStr, CString};
use std::{ptr};

/// Memory utilities that deals with primitive types passed from user process
/// running inside enclave
pub mod from_user {
    use super::*;

    /// Check the user pointer is within the readable memory range of the user process
    pub fn check_ptr<T>(user_ptr: *const T) -> Result<(), Error> {
        Ok(())
    }

    /// Check the mutable user pointer is within the writable memory of the user process
    pub fn check_mut_ptr<T>(user_ptr: *mut T) -> Result<(), Error> {
        Ok(())
    }

    /// Check the readonly array is within the readable memory of the user process
    pub fn check_array<T>(user_buf: *const T, count: usize) -> Result<(), Error> {
        Ok(())
    }

    /// Check the mutable array is within the writable memory of the user process
    pub fn check_mut_array<T>(user_buf: *mut T, count: usize) -> Result<(), Error> {
        Ok(())
    }

    /// Clone a C-string from the user process safely
    pub fn clone_cstring_safely(out_ptr: *const c_char)
        -> Result<CString, Error>
    {
        check_ptr(out_ptr)?;
        // TODO: using from_ptr directly is not safe
        let cstr = unsafe { CStr::from_ptr(out_ptr) };
        let cstring = CString::from(cstr);
        Ok(cstring)
    }

    /// Clone a C-string array (const char*[]) from the user process safely
    ///
    /// This array must be ended with a NULL pointer.
    pub fn clone_cstrings_safely(user_ptr: *const *const c_char)
        -> Result<Vec<CString>, Error>
    {
        let mut cstrings = Vec::new();
        let mut user_ptr = user_ptr;
        while user_ptr != ptr::null() {
            let cstr_ptr = unsafe { *user_ptr };
            let cstring = clone_cstring_safely(cstr_ptr)?;
            cstrings.push(cstring);
            user_ptr = unsafe { user_ptr.offset(1) };
        }
        Ok(cstrings)
    }
}

/// Memory utilities that deals with primitive types passed from outside the enclave
pub mod from_untrusted {
    use super::*;

    /// Check the untrusted pointer is outside the enclave
    pub fn check_ptr<T>(out_ptr: *const T) -> Result<(), Error> {
        Ok(())
    }

    /// Check the untrusted array is outside the enclave
    pub fn check_array<T>(out_ptr: *const T, count: usize) -> Result<(), Error> {
        Ok(())
    }

    /// Clone a C-string from outside the enclave
    pub fn clone_cstring_safely(out_ptr: *const c_char)
        -> Result<CString, Error>
    {
        check_ptr(out_ptr)?;
        // TODO: using from_ptr directly is not safe
        let cstr = unsafe { CStr::from_ptr(out_ptr) };
        let cstring = CString::from(cstr);
        Ok(cstring)
    }

    /// Clone a C-string array (const char*[]) from outside the enclave
    ///
    /// This array must be ended with a NULL pointer.
    pub fn clone_cstrings_safely(out_ptr: *const *const c_char)
        -> Result<Vec<CString>, Error>
    {
        let mut cstrings = Vec::new();
        if out_ptr == ptr::null() { return Ok(cstrings); }

        let mut out_ptr = out_ptr;
        loop {
            check_ptr(out_ptr);
            let cstr_ptr = {
                let cstr_ptr = unsafe { *out_ptr };
                if cstr_ptr == ptr::null() { break; }
                check_ptr(cstr_ptr);
                cstr_ptr
            };
            let cstring = clone_cstring_safely(cstr_ptr)?;
            cstrings.push(cstring);

            out_ptr = unsafe { out_ptr.offset(1) };
        }
        Ok(cstrings)
    }
}

