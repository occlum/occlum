use std::cell::Cell;

/// A marker trait for types that can be used in untrusted memory
/// in a relatively safe way.
///
/// # Overview
///
/// Not all types are created equal: some of them are difficult---
/// if not impossible---to be used safely when residing in untrusted memory.
/// One obvious class is heap-backed Rust containers, like `Vec<T>`.
/// If such types were put into untrusted memory, then an attacker could manipulate
/// the internal pointer. Enums, in general, cannot be used safely in untrusted
/// memory since an attacker could break Rust compiler's assumption on its memory
/// representation, thus causing undefined behaviors. Surprisingly, even a primitive
/// type like `bool` is dangerous to use in untrusted memory. This is also
/// due to Rust compiler's strong assumption on memory representation.
///
/// Here is a list of core types that implements `MaybeUntrusted`:
/// * Primitive types: `u8`, `u16`, `u32`, `u64`, `usize` and their signed counterparts;
/// * Pointer types: `*const T` and `*mut T`, where `T: MaybeUntrusted`;
/// * Array types: `[T; N]`, where `T: MaybeUntrusted` and 1<= `N` <= 32;
/// * Core types: `Cell<T>`, where `T: MaybeUntrusted`.
/// * Libc types: Most C-style structs defined in libc can have `MaybeUntrusted` implemented.
/// But since there are simply too many of them yet most of them are irrelevant for our usage,
/// it is more reasonable to implement `MaybeUntrusted` on demand. Currently, the list of libc
/// types that have implemented `MaybeUntrusted` is pretty short:
///     * `sockaddr_storage`.
///
/// For user-defined types, the `MaybeUntrusted` trait should be implemented with discretion.
/// A good rule of thumb is only implementing `MaybeUntrusted` for C-style structs.
///
/// # Must implement `Sized`
///
/// The `MaybeUntrusted` trait requires `Sized`. In other words, it wouldn't be safe to
/// store any DST values in untrusted memory since the length part of DST may be tampered by
/// attacker, leading to buffer overflow.
///
/// # Safe to be uninitialized
///
/// Another implicit property of `MaybeUntrusted` types is that unlike most Rust types it is
/// perfectly fine to instantiate _uninitialized_ values for a `MaybeUntrusted` type. After all,
/// our thread model assumes an attacker that may tamper with the values of `MaybeUntrusted`
/// types in an arbitrary way. The code that handles `MaybeUntrusted` types must be robust
/// enough to deal with any possible values, including uninitialized ones.
///
/// As a result, `MaybeUntrusted` comes with two convenient constructor methods that creates
/// an instance whose value is uninitialized or zeroed, respectively.
///
/// # Good to have `Copy`
///
/// As an effective defense against against TOCTOU attacks, a common practice is to---before
/// actually using a value of `MaybeUntrusted`---first copy the value into trusted memory and
/// validate its value. Thus, most types that implement `MaybeUntrusted` should also implement
/// `Copy`.
use std::mem::MaybeUninit;

pub unsafe trait MaybeUntrusted: Sized {
    fn uninit() -> Self {
        unsafe { MaybeUninit::uninit().assume_init() }
    }

    fn zeroed() -> Self {
        unsafe { MaybeUninit::zeroed().assume_init() }
    }
}

macro_rules! impl_maybe_untrusted {
    ($($type:ty),*) => {
        $(
            unsafe impl MaybeUntrusted for $type {}
        )*
    };
}

impl_maybe_untrusted! {
    // Primitive types
    u8,
    u16,
    u32,
    u64,
    usize,
    i8,
    i16,
    i32,
    i64,
    isize,

    // Libc types
    libc::sockaddr_storage,
    libc::iovec
}

unsafe impl<T: MaybeUntrusted> MaybeUntrusted for *const T {}

unsafe impl<T: MaybeUntrusted> MaybeUntrusted for *mut T {}

unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 1] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 2] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 3] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 4] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 5] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 6] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 7] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 8] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 9] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 10] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 11] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 12] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 13] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 14] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 15] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 16] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 17] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 18] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 19] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 20] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 21] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 22] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 23] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 24] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 25] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 26] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 27] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 28] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 29] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 30] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 31] {}
unsafe impl<T: MaybeUntrusted> MaybeUntrusted for [T; 32] {}

unsafe impl<T: MaybeUntrusted> MaybeUntrusted for Cell<T> {}
