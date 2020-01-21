use super::*;
use rcore_fs_sefs::dev::{SefsUuid, UuidProvider};
use sgx_types::sgx_status_t;

extern "C" {
    fn sgx_read_rand(rand_buf: *mut u8, buf_size: usize) -> sgx_status_t;
}

pub struct SgxUuidProvider;

impl UuidProvider for SgxUuidProvider {
    fn generate_uuid(&self) -> SefsUuid {
        let mut uuid: [u8; 16] = [0u8; 16];
        let buf = uuid.as_mut_ptr();
        let size = 16;
        let status = unsafe { sgx_read_rand(buf, size) };
        assert!(status == sgx_status_t::SGX_SUCCESS);
        SefsUuid(uuid)
    }
}
