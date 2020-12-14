use super::*;
pub use sgx_types::{sgx_ql_qv_result_t, sgx_quote3_error_t, sgx_report_data_t, sgx_target_info_t};

pub struct QuoteGenerator {
    qe_target_info: sgx_target_info_t,
    quote_size: u32,
}

impl QuoteGenerator {
    pub fn new() -> Self {
        let mut qe_target_info = sgx_target_info_t::default();
        let mut quote_size: u32 = 0;

        unsafe {
            let mut qe3_ret = sgx_quote3_error_t::SGX_QL_SUCCESS;
            let sgx_status = occlum_ocall_init_dcap_quote_generator(
                &mut qe3_ret,
                &mut qe_target_info,
                &mut quote_size,
            );
            assert_eq!(sgx_status_t::SGX_SUCCESS, sgx_status);
            assert_eq!(
                sgx_quote3_error_t::SGX_QL_SUCCESS,
                qe3_ret,
                "fail to launch QE"
            );
        }

        Self {
            qe_target_info,
            quote_size,
        }
    }

    pub fn get_quote_size(&self) -> u32 {
        self.quote_size
    }

    pub fn generate_quote(&self, report_data: &sgx_report_data_t) -> Result<Vec<u8>> {
        let mut quote = vec![0; self.quote_size as usize];
        let mut qe3_ret = sgx_quote3_error_t::SGX_QL_SUCCESS;
        let app_report = create_report(Some(&self.qe_target_info), Some(report_data))?;

        unsafe {
            let sgx_status = occlum_ocall_generate_dcap_quote(
                &mut qe3_ret,
                &app_report,
                self.quote_size,
                quote.as_mut_ptr(),
            );
            assert_eq!(sgx_status_t::SGX_SUCCESS, sgx_status);
        }

        match qe3_ret {
            sgx_quote3_error_t::SGX_QL_SUCCESS => Ok(quote),
            sgx_quote3_error_t::SGX_QL_ERROR_BUSY => {
                return_errno!(EBUSY, "occlum_ocall_sgx_gen_dcap_quote is temporarily busy");
            }
            _ => return_errno!(EINVAL, "occlum_ocall_sgx_gen_dcap_quote failed"),
        }
    }
}

extern "C" {
    fn occlum_ocall_init_dcap_quote_generator(
        ret: *mut sgx_quote3_error_t,
        qe_target_info: *mut sgx_target_info_t,
        quote_size: *mut uint32_t,
    ) -> sgx_status_t;
    fn occlum_ocall_generate_dcap_quote(
        ret: *mut sgx_quote3_error_t,
        app_report: *const sgx_report_t,
        quote_size: uint32_t,
        quote_buf: *mut uint8_t,
    ) -> sgx_status_t;
}
