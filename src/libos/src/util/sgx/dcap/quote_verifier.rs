use super::*;

#[derive(Copy, Clone)]
pub struct QuoteVerifier {
    supplemental_data_size: u32,
}

// The latest QvE ISVSVN from Intel PCS.
// It should be updated when a newer QvE is released.
const QVE_ISVSVN_THRESHOLD: sgx_isv_svn_t = 5;

impl QuoteVerifier {
    pub fn new() -> Option<Self> {
        let mut supplemental_data_size = 0;
        let mut sgx_status = sgx_status_t::SGX_SUCCESS;
        unsafe {
            sgx_status = occlum_ocall_get_supplement_size(&mut supplemental_data_size);
        }

        if sgx_status != sgx_status_t::SGX_SUCCESS || supplemental_data_size == 0 {
            error!("DCAP Quote Verifier new failed {}", sgx_status);
            None
        } else {
            Some(Self {
                supplemental_data_size,
            })
        }
    }

    pub fn get_supplemental_data_size(&self) -> u32 {
        self.supplemental_data_size
    }

    pub fn verify_quote(&self, quote: &[u8]) -> Result<(u32, sgx_ql_qv_result_t, Vec<u8>)> {
        let mut qe3_ret = sgx_quote3_error_t::SGX_QL_SUCCESS;
        // FIXME: a trusted time should be provided here in production mode
        let current_time = time::do_gettimeofday().as_duration().as_secs() as time_t;
        let mut quote_verification_result = sgx_ql_qv_result_t::SGX_QL_QV_RESULT_OK;
        let mut collateral_expiration_status = 1;
        let mut supplemental_data = vec![0; self.supplemental_data_size as usize];
        let mut qve_report_info = sgx_ql_qe_report_info_t::default();
        let mut nonce;

        unsafe {
            let sgx_status = sgx_read_rand(
                qve_report_info.nonce.rand.as_mut_ptr(),
                qve_report_info.nonce.rand.len(),
            );
            if sgx_status != sgx_status_t::SGX_SUCCESS {
                return_errno!(EAGAIN, "failed to get random number from sgx");
            }
            nonce = qve_report_info.nonce;
        }

        qve_report_info.app_enclave_target_info = get_self_target()?;

        unsafe {
            let sgx_status = occlum_ocall_verify_dcap_quote(
                &mut qe3_ret,
                quote.as_ptr(),
                quote.len() as u32,
                std::ptr::null(),
                current_time,
                &mut collateral_expiration_status,
                &mut quote_verification_result,
                &mut qve_report_info,
                supplemental_data.len() as u32,
                supplemental_data.as_mut_ptr(),
            );
            assert_eq!(sgx_status_t::SGX_SUCCESS, sgx_status);
            // We have to re-write qve_report_info.nonce with the value we backed up earlier,
            // since qve_report_info.nonce can be overwrite by attacker from ocall side.
            qve_report_info.nonce = nonce;
        }

        match qe3_ret {
            sgx_quote3_error_t::SGX_QL_SUCCESS => {
                let qe3_ret = unsafe {
                    sgx_tvl_verify_qve_report_and_identity(
                        quote.as_ptr(),
                        quote.len() as u32,
                        &qve_report_info,
                        current_time,
                        collateral_expiration_status,
                        quote_verification_result,
                        supplemental_data.as_ptr(),
                        supplemental_data.len() as u32,
                        QVE_ISVSVN_THRESHOLD,
                    )
                };
                if qe3_ret == sgx_quote3_error_t::SGX_QL_SUCCESS {
                    Ok((
                        collateral_expiration_status,
                        quote_verification_result,
                        supplemental_data,
                    ))
                } else {
                    debug!("returned qe3 error is {}", qe3_ret);
                    return_errno!(EINVAL, "failed to verify quote");
                }
            }
            sgx_quote3_error_t::SGX_QL_ERROR_BUSY => {
                return_errno!(EBUSY, "occlum_ocall_sgx_ver_dcap_quote is temporarily busy");
            }
            _ => return_errno!(EINVAL, "occlum_ocall_sgx_ver_dcap_quote failed"),
        }
    }
}

extern "C" {
    fn occlum_ocall_get_supplement_size(size: *mut u32) -> sgx_status_t;
    // sgx_ql_qve_collateral_t uses char that is not FFI-safe. It will raise improper_ctypes
    // warning. As only char pointer is used, we allow the use here.
    #[allow(improper_ctypes)]
    fn occlum_ocall_verify_dcap_quote(
        ret: *mut sgx_quote3_error_t,
        quote_buf: *const uint8_t,
        quote_size: uint32_t,
        quote_collateral: *const sgx_ql_qve_collateral_t,
        expiration_check_date: time_t,
        collateral_expiration_status: *mut uint32_t,
        quote_verification_result: *mut sgx_ql_qv_result_t,
        qve_report_info: *mut sgx_ql_qe_report_info_t,
        supplemental_data_size: uint32_t,
        supplemental_data: *mut uint8_t,
    ) -> sgx_status_t;
}
