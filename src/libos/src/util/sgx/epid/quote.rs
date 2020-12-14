//! SGX EPID Quote in a memory safe representation and with hash validation.

use super::*;

#[derive(Debug, Default)]
pub struct Quote {
    quote_buf: Vec<u8>,
}

impl Quote {
    pub fn new(
        quote_raw_buf: &[u8],
        quote_nonce: &sgx_quote_nonce_t,
        qe_report: &sgx_report_t,
    ) -> Result<Self> {
        let quote_buf = Self::new_buf(quote_raw_buf)?;
        Self::validate_quote_buf(&quote_buf, quote_nonce, qe_report)?;
        Ok(Self { quote_buf })
    }

    fn new_buf(quote_raw_buf: &[u8]) -> Result<Vec<u8>> {
        if quote_raw_buf.len() < std::mem::size_of::<sgx_quote_t>() {
            return_errno!(EINVAL, "buffer is too small for SGX quote itself");
        }
        let quote = unsafe { &*(quote_raw_buf.as_ptr() as *const sgx_quote_t) };
        let quote_size = std::mem::size_of::<sgx_quote_t>() + quote.signature_len as usize;
        if quote_size > quote_raw_buf.len() {
            return_errno!(EINVAL, "buffer is too small for SGX quote with signature");
        }
        let quote_buf = quote_raw_buf[..quote_size].to_vec();
        Ok(quote_buf)
    }

    fn validate_quote_buf(
        quote_buf: &[u8],
        quote_nonce: &sgx_quote_nonce_t,
        qe_report: &sgx_report_t,
    ) -> Result<()> {
        // According to Intel manual:
        // The purpose of QE report (`qe_report`) is for the ISV enclave to confirm the
        // quote (i.e., `quote_buf`) received is not modified by the untrusted SW stack,
        // and not a replay. The implementation in QE is to generate a report targeting
        // the ISV enclave (i.e., Occlum's enclave), with the lower 32 bytes in
        // QE report (i.e., `qe_reprot.data.d[..32]`) equivalent to SHA256(quote_nonce||quote_buf).
        let expected_hash = &qe_report.body.report_data.d[..32];
        let actual_hash = {
            let mut quote_nonce_and_buf: Vec<u8> = quote_nonce
                .rand
                .iter()
                .chain(quote_buf.iter())
                .cloned()
                .collect();
            sgx_tcrypto::rsgx_sha256_slice(&quote_nonce_and_buf).unwrap()
        };
        if expected_hash != actual_hash {
            return_errno!(EINVAL, "invalid quote (unexpected hash)");
        }
        Ok(())
    }

    pub fn get_fields(&self) -> &sgx_quote_t {
        unsafe { &*(self.quote_buf.as_ptr() as *const sgx_quote_t) }
    }

    pub fn get_signature(&self) -> &[u8] {
        &self.quote_buf[std::mem::size_of::<sgx_quote_t>()..]
    }

    pub fn get_size(&self) -> usize {
        self.quote_buf.len()
    }

    pub fn dump_to_buf(&self, dst_buf: &mut [u8]) -> Result<()> {
        let src_buf = &self.quote_buf;
        if src_buf.len() > dst_buf.len() {
            return_errno!(
                EINVAL,
                "the given output buffer for quote is NOT big enough"
            );
        }
        dst_buf[..src_buf.len()].copy_from_slice(src_buf);
        Ok(())
    }
}
