use self::cpuid::*;
use self::rdtsc::*;
use super::*;
use sgx_types::*;

pub fn register_exception_handlers() {
    setup_cpuid_info();
    unsafe {
        sgx_register_exception_handler(1, handle_cpuid_exception);
        sgx_register_exception_handler(1, handle_rdtsc_exception);
    }
}

mod cpuid;
mod rdtsc;
