use crate::prelude::*;

#[repr(C, align(64))]
#[derive(Default, Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct sgx_interrupt_info_t {
    pub cpu_context: sgx_cpu_context_t,
    pub interrupt_valid: uint32_t,
    reserved: uint32_t,
    pub xsave_size: uint64_t,
    pub reserved1: [uint64_t; 4],
    pub xsave_area: [uint8_t; 0],
}

#[allow(non_camel_case_types)]
pub type sgx_interrupt_handler_t = extern "C" fn(info: *mut sgx_interrupt_info_t) -> int32_t;

extern "C" {
    pub fn sgx_interrupt_init(handler: sgx_interrupt_handler_t) -> sgx_status_t;
    pub fn sgx_interrupt_enable(code_addr: usize, code_size: usize) -> sgx_status_t;
    pub fn sgx_interrupt_disable() -> sgx_status_t;
}
