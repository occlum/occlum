use super::*;

use sgx_types::sgx_status_t;

const MAX_RETRIES: u32 = 50;

pub fn get_random(rand: &mut [u8]) -> Result<()> {
    extern "C" {
        fn sgx_read_rand(rand_buf: *mut u8, buf_size: usize) -> sgx_status_t;
    }

    if rand.is_empty() {
        return Ok(());
    }
    // sgx_read_rand() may fail because of HW failure of RDRAND instruction,
    // add retries to get the random number.
    for _ in 0..MAX_RETRIES {
        let status = unsafe { sgx_read_rand(rand.as_mut_ptr(), rand.len()) };
        match status {
            sgx_status_t::SGX_SUCCESS => {
                return Ok(());
            }
            sgx_status_t::SGX_ERROR_INVALID_PARAMETER => {
                panic!("invalid argument to get random number from SGX");
            }
            _ => {}
        }
    }
    Err(errno!(EAGAIN, "failed to get random number from SGX"))
}
