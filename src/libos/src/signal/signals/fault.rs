use super::super::c_types::*;
use super::super::constants::*;
use super::super::{SigNum, Signal};
use crate::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct FaultSignal {
    num: SigNum,
    code: i32,
}

impl FaultSignal {
    pub fn new(info: &sgx_exception_info_t) -> Self {
        // FIXME: the following mapping from exception to signal is not accurate.
        use sgx_exception_vector_t::*;
        let (num, code) = match info.exception_vector {
            // Divider exception
            SGX_EXCEPTION_VECTOR_DE => (SIGFPE, FPE_INTDIV),
            // Floating-point exception
            SGX_EXCEPTION_VECTOR_MF |
            // SIMD floating-point exception
            SGX_EXCEPTION_VECTOR_XM => (SIGFPE, FPE_FLTDIV),
            // Invalid opcode exception
            SGX_EXCEPTION_VECTOR_UD |
            // Debug exception: should not occur in enclave; treat is as #UD
            SGX_EXCEPTION_VECTOR_DB |
            // Break point exception: should not occur in enclave; treat is as #UD
            SGX_EXCEPTION_VECTOR_BP => (SIGILL, ILL_ILLOPC),
            // Bound range exception
            SGX_EXCEPTION_VECTOR_BR => (SIGSEGV, SEGV_BNDERR),
            // Alignment check exception
            SGX_EXCEPTION_VECTOR_AC => (SIGBUS, BUS_ADRALN),
            // TODO: handle page fault and general protection exceptions
            _ => panic!("illegal exception: cannot be converted to signal"),
        };
        Self { num, code }
    }
}

impl Signal for FaultSignal {
    fn num(&self) -> SigNum {
        self.num
    }

    fn to_info(&self) -> siginfo_t {
        let info = siginfo_t::new(self.num, self.code);
        // TODO: set info.si_addr
        info
    }
}
