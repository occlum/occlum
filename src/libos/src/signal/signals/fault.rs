use super::super::c_types::*;
use super::super::constants::*;
use super::super::{SigNum, Signal};
use crate::entry::exception::Exception;
use crate::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct FaultSignal {
    num: SigNum,
    code: i32,
    addr: Option<u64>,
}

impl FaultSignal {
    pub fn new(exception: &Exception) -> Self {
        // TODO: the current mapping from exception to signal is only a first
        // order approximation. The resulting signum or siginfo may not be
        // identical to Linux's behavior.
        use sgx_exception_vector_t::*;
        let (num, code, addr) = match exception.vector {
            // Divider exception
            SGX_EXCEPTION_VECTOR_DE => (SIGFPE, FPE_INTDIV, None),
            // Floating-point exception
            SGX_EXCEPTION_VECTOR_MF |
            // SIMD floating-point exception
            SGX_EXCEPTION_VECTOR_XM => (SIGFPE, FPE_FLTDIV, None),
            // Invalid opcode exception
            SGX_EXCEPTION_VECTOR_UD |
            // Debug exception: should not occur in enclave; treat is as #UD
            SGX_EXCEPTION_VECTOR_DB |
            // Break point exception: should not occur in enclave; treat is as #UD
            SGX_EXCEPTION_VECTOR_BP => (SIGILL, ILL_ILLOPC, None),
            // Bound range exception
            SGX_EXCEPTION_VECTOR_BR => (SIGSEGV, SEGV_BNDERR, None),
            // Alignment check exception
            SGX_EXCEPTION_VECTOR_AC => (SIGBUS, BUS_ADRALN, None),
            // Page fault exception
            SGX_EXCEPTION_VECTOR_PF => {
                const PF_ERR_FLAG_PRESENT : u32 = 1u32 << 0;
                let code = if exception.exinfo.errcd & PF_ERR_FLAG_PRESENT != 0 {
                    SEGV_ACCERR
                } else {
                    SEGV_MAPERR
                };
                let addr = Some(exception.exinfo.maddr);
                (SIGSEGV, code, addr)
            },
            // General protection exception
            SGX_EXCEPTION_VECTOR_GP => (SIGBUS, BUS_ADRERR, None),
            _ => panic!("exception cannot be converted to signal"),
        };
        Self { num, code, addr }
    }

    pub fn addr(&self) -> Option<u64> {
        self.addr
    }
}

impl Signal for FaultSignal {
    fn num(&self) -> SigNum {
        self.num
    }

    fn to_info(&self) -> siginfo_t {
        let mut info = siginfo_t::new(self.num, self.code);
        info.set_si_addr(self.addr.unwrap_or_default() as *const c_void);
        info
    }
}
