//! SGX EPID group ID retrieval and quote generation.

use super::*;

pub struct AttestationAgent {
    inner: Option<InnerAgent>,
}

impl AttestationAgent {
    pub fn new() -> Self {
        Self { inner: None }
    }

    pub fn get_epid_group_id(&mut self) -> Result<sgx_epid_group_id_t> {
        self.init_inner()?;
        self.inner.as_mut().unwrap().get_epid_group_id()
    }

    pub fn generate_quote(
        &mut self,
        sigrl: Option<&[u8]>,
        report_data: &sgx_report_data_t,
        quote_type: sgx_quote_sign_type_t,
        spid: &sgx_spid_t,
        nonce: &sgx_quote_nonce_t,
    ) -> Result<Quote> {
        self.init_inner()?;
        self.inner
            .as_mut()
            .unwrap()
            .generate_quote(sigrl, report_data, quote_type, spid, nonce)
    }

    fn init_inner(&mut self) -> Result<()> {
        if self.inner.is_none() {
            let inner = InnerAgent::new()?;
            self.inner = Some(inner);
        }
        Ok(())
    }
}

struct InnerAgent {
    target_info: sgx_target_info_t,
    epid_group_id: sgx_epid_group_id_t,
}

impl InnerAgent {
    pub fn new() -> Result<Self> {
        let (target_info, epid_group_id) = Self::init_fields()?;
        Ok(Self {
            target_info,
            epid_group_id,
        })
    }

    fn init_fields() -> Result<(sgx_target_info_t, sgx_epid_group_id_t)> {
        extern "C" {
            pub fn occlum_ocall_sgx_init_quote(
                retval: *mut sgx_status_t,
                target_info: *mut sgx_target_info_t,
                epid_group_id: *mut sgx_epid_group_id_t,
            ) -> sgx_status_t;
        }

        let mut target_info = Default::default();
        let mut epid_group_id = Default::default();
        unsafe {
            let mut retval = Default::default();
            let status = occlum_ocall_sgx_init_quote(
                &mut retval as *mut sgx_status_t,
                &mut target_info as *mut sgx_target_info_t,
                &mut epid_group_id as *mut sgx_epid_group_id_t,
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);

            if retval != sgx_status_t::SGX_SUCCESS {
                match retval {
                    sgx_status_t::SGX_ERROR_BUSY => {
                        return_errno!(EBUSY, "occlum_ocall_sgx_init_quote is temporarily busy")
                    }
                    _ => return_errno!(EINVAL, "occlum_ocall_sgx_init_quote failed"),
                }
            }
        }

        Ok((target_info, epid_group_id))
    }

    pub fn get_epid_group_id(&self) -> Result<sgx_epid_group_id_t> {
        Ok(self.epid_group_id)
    }

    pub fn generate_quote(
        &mut self,
        sigrl: Option<&[u8]>,
        report_data: &sgx_report_data_t,
        quote_type: sgx_quote_sign_type_t,
        spid: &sgx_spid_t,
        nonce: &sgx_quote_nonce_t,
    ) -> Result<Quote> {
        extern "C" {
            pub fn occlum_ocall_sgx_get_epid_quote(
                retval: *mut sgx_status_t,         // Output
                sigrl: *const u8,                  // Input (optional)
                sigrl_len: u32,                    // Input (optional)
                report: *const sgx_report_t,       // Input
                quote_type: sgx_quote_sign_type_t, // Input
                spid: *const sgx_spid_t,           // Input
                nonce: *const sgx_quote_nonce_t,   // Input
                qe_report: *mut sgx_report_t,      // Output
                quote_buf_ptr: *mut u8,            // Output
                quote_buf_len: u32,                // Input
            ) -> sgx_status_t;
            fn occlum_ocall_sgx_calc_quote_size(
                p_retval: *mut sgx_status_t,
                p_sig_rl: *const u8,
                sig_rl_size: u32,
                p_quote_size: *mut u32,
            ) -> sgx_status_t;
        }

        // Prepare arguments for OCall
        let (sigrl_ptr, sigrl_size): (*const u8, u32) = {
            match sigrl {
                Some(sigrl) => {
                    let sigrl_ptr = sigrl.as_ptr();
                    let sigrl_size = {
                        if sigrl.len() > std::u32::MAX as usize {
                            return_errno!(EINVAL, "sigrl is too large");
                        }
                        sigrl.len() as u32
                    };
                    (sigrl_ptr, sigrl_size)
                }
                None => (std::ptr::null(), 0),
            }
        };
        let report = rsgx_create_report(&self.target_info, report_data)
            .map_err(|_e| errno!(EINVAL, "sgx_error"))?;
        let mut qe_report = sgx_report_t::default();
        let mut quote_len: u32 = 0;
        let mut rt = Default::default();
        let status = unsafe {
            occlum_ocall_sgx_calc_quote_size(
                &mut rt as _,
                sigrl_ptr,
                sigrl_size,
                &mut quote_len as _,
            )
        };
        assert!(status == sgx_status_t::SGX_SUCCESS);
        if rt != sgx_status_t::SGX_SUCCESS {
            return_errno!(EINVAL, "occlum_ocall_sgx_calc_quote_size failed");
        }
        let mut quote_buf = vec![0_u8; quote_len as usize];

        // Do OCall
        unsafe {
            let mut retval = Default::default();
            let status = occlum_ocall_sgx_get_epid_quote(
                &mut retval as *mut sgx_status_t,
                sigrl_ptr,
                sigrl_size,
                &report as *const sgx_report_t,
                quote_type,
                spid as *const sgx_spid_t,
                nonce as *const sgx_quote_nonce_t,
                &mut qe_report as *mut sgx_report_t,
                quote_buf.as_mut_ptr() as *mut u8,
                quote_buf.len() as u32,
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);

            if retval != sgx_status_t::SGX_SUCCESS {
                match retval {
                    sgx_status_t::SGX_ERROR_BUSY => {
                        return_errno!(EBUSY, "occlum_ocall_sgx_get_epid_quote is temporarily busy")
                    }
                    _ => return_errno!(EINVAL, "occlum_ocall_sgx_get_epid_quote failed"),
                }
            }
        }

        // Make sure the QE report is valid
        SgxQeReportValidator::new(&self.target_info, nonce).validate(&qe_report)?;

        // Construct the resulting quote
        let quote = Quote::new(&quote_buf, &nonce, &qe_report)?;

        Ok(quote)
    }
}

/// Validating SGX Quoting Enclave (QE) report.
struct SgxQeReportValidator<'a> {
    target_info: &'a sgx_target_info_t,
    nonce: &'a sgx_quote_nonce_t,
}

impl<'a> SgxQeReportValidator<'a> {
    pub fn new(target_info: &'a sgx_target_info_t, nonce: &'a sgx_quote_nonce_t) -> Self {
        SgxQeReportValidator { target_info, nonce }
    }

    pub fn validate(&self, qe_report: &sgx_report_t) -> Result<()> {
        self.validate_integrity(qe_report)?;
        self.validate_platform(qe_report)?;
        Ok(())
    }

    fn validate_integrity(&self, qe_report: &sgx_report_t) -> Result<()> {
        rsgx_verify_report(qe_report)
            .map_err(|_e| errno!(EINVAL, "quote report is NOT authentic"))?;
        Ok(())
    }

    fn validate_platform(&self, qe_report: &sgx_report_t) -> Result<()> {
        if self.target_info.mr_enclave.m != qe_report.body.mr_enclave.m
            || self.target_info.attributes.flags != qe_report.body.attributes.flags
            || self.target_info.attributes.xfrm != qe_report.body.attributes.xfrm
        {
            return_errno!(EINVAL, "quote report is NOT produced on the same platform");
        }
        Ok(())
    }
}
