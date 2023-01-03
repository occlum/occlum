use super::*;

use std::ptr;

pub fn get_key(key_request: &sgx_key_request_t) -> Result<sgx_key_128bit_t> {
    let mut key = sgx_key_128bit_t::default();
    let sgx_status = unsafe { sgx_get_key(key_request, &mut key as *mut sgx_key_128bit_t) };
    match sgx_status {
        sgx_status_t::SGX_SUCCESS => Ok(key),
        sgx_status_t::SGX_ERROR_INVALID_PARAMETER => return_errno!(EINVAL, "invalid paramters"),
        _ => {
            error!("sgx_get_key return {:?}", sgx_status);
            return_errno!(EINVAL, "unexpected SGX error")
        }
    }
}
