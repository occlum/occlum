use super::*;

pub fn do_getrandom(rand_buf: &mut [u8], flags: RandFlags) -> Result<()> {
    debug!("getrandom: flags: {:?}", flags);
    if flags.contains(RandFlags::GRND_NONBLOCK) {
        get_random(rand_buf)
    } else {
        get_random_blocking(rand_buf)
    }
}

bitflags! {
    pub struct RandFlags: u32 {
        /// Don't block and return EAGAIN instead
        const GRND_NONBLOCK = 0x0001;
        /// No effect
        const GRND_RANDOM = 0x0002;
    }
}

fn get_random_blocking(rand: &mut [u8]) -> Result<()> {
    loop {
        if get_random(rand).is_ok() {
            break;
        }
    }
    Ok(())
}

pub fn get_random(rand: &mut [u8]) -> Result<()> {
    use sgx_types::sgx_status_t;
    extern "C" {
        fn sgx_read_rand(rand_buf: *mut u8, buf_size: usize) -> sgx_status_t;
    }
    const MAX_TIMES: u32 = 50;

    if rand.is_empty() {
        return Ok(());
    }
    // sgx_read_rand() may fail because of HW failure of RDRAND instruction,
    // add retries to get the random number.
    for _ in 0..MAX_TIMES {
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
