//! Allocation and access of untrusted memory in a _safe_ way.

#![cfg_attr(feature = "sgx", no_std)]

cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        #[macro_use]
        extern crate sgx_tstd as std;
        extern crate sgx_libc as libc;
    }
}

mod maybe_untrusted;
pub use maybe_untrusted::MaybeUntrusted;

mod box_;
pub use box_::UntrustedBox;
