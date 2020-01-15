/// Manipulate and access untrusted memory or functionalities safely
mod alloc;
mod slice_alloc;
mod slice_ext;

use super::*;

pub use self::alloc::UNTRUSTED_ALLOC;
pub use self::slice_alloc::UntrustedSliceAlloc;
pub use self::slice_ext::{SliceAsMutPtrAndLen, SliceAsPtrAndLen};
