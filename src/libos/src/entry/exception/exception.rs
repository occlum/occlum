use std::ops::Deref;

use crate::prelude::*;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Exception {
    pub type_: sgx_exception_type_t,
    pub vector: sgx_exception_vector_t,
    pub exinfo: sgx_misc_exinfo_t,
}

impl<T> From<T> for Exception
where
    T: Deref<Target = sgx_exception_info_t>,
{
    fn from(sgx_info: T) -> Self {
        Self {
            type_: sgx_info.exception_type,
            vector: sgx_info.exception_vector,
            exinfo: sgx_info.exinfo,
        }
    }
}

impl std::fmt::Debug for Exception {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Exception")
            .field("type_", &(self.type_ as u16))
            .field("vector", &(self.vector as u16))
            .field("exinfo", &"<omitted>")
            .finish()
    }
}
