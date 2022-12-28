use super::*;

use std::ptr;

pub fn get_self_target() -> Result<sgx_target_info_t> {
    let mut self_target = sgx_target_info_t::default();
    let sgx_status = unsafe { sgx_self_target(&mut self_target) };
    match sgx_status {
        sgx_status_t::SGX_SUCCESS => Ok(self_target),
        _ => return_errno!(EINVAL, "unexpected SGX error"),
    }
}

pub fn create_report(
    target_info: Option<&sgx_target_info_t>,
    report_data: Option<&sgx_report_data_t>,
) -> Result<sgx_report_t> {
    let mut report = sgx_report_t::default();
    let sgx_status = unsafe {
        sgx_create_report(
            target_info.map_or(ptr::null(), |t| t),
            report_data.map_or(ptr::null(), |t| t),
            &mut report,
        )
    };
    match sgx_status {
        sgx_status_t::SGX_SUCCESS => Ok(report),
        sgx_status_t::SGX_ERROR_INVALID_PARAMETER => return_errno!(EINVAL, "invalid parameters"),
        _ => return_errno!(EINVAL, "unexpected SGX error"),
    }
}

pub fn verify_report(report: &sgx_report_t) -> Result<()> {
    let sgx_status = unsafe { sgx_verify_report(report) };
    match sgx_status {
        sgx_status_t::SGX_SUCCESS => Ok(()),
        sgx_status_t::SGX_ERROR_MAC_MISMATCH => return_errno!(EINVAL, "report MAC mismatch"),
        sgx_status_t::SGX_ERROR_INVALID_PARAMETER => return_errno!(EINVAL, "invalid report"),
        _ => return_errno!(EINVAL, "unexpected SGX error"),
    }
}
