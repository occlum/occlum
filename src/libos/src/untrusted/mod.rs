/// Manipulate and access untrusted memory or functionalities safely
mod alloc;
mod slice_alloc;
mod slice_ext;
mod untrusted_circular_buf;

use super::*;

pub use self::alloc::UNTRUSTED_ALLOC;
pub use self::slice_alloc::{UntrustedSlice, UntrustedSliceAlloc, UntrustedSliceAllocGuard};
pub use self::slice_ext::{SliceAsMutPtrAndLen, SliceAsPtrAndLen};
pub use self::untrusted_circular_buf::UntrustedCircularBuf;
