use crate::prelude::*;
use std::mem::{size_of, size_of_val, MaybeUninit};
/// A trait for all C version of C socket addresses.
///
/// There are four types that implement this trait:
/// * `libc::sockaddr_in`
/// * `(libc::sockaddr_in, usize)`
/// * `(libc::sockaddr_un, usize)`
/// * `(libc::sockaddr_storage, usize)`.
pub trait CSockAddr {
    /// The network family of the address.
    fn c_family(&self) -> libc::sa_family_t;

    /// The address in bytes (excluding the family part).
    fn c_addr(&self) -> &[u8];

    /// Returns the address in `libc::sockaddr_storage` along with its length.
    fn to_c_storage(&self) -> (libc::sockaddr_storage, usize) {
        let mut c_storage =
            unsafe { MaybeUninit::<libc::sockaddr_storage>::uninit().assume_init() };

        c_storage.ss_family = self.c_family();
        let offset = size_of_val(&c_storage.ss_family);

        let c_storage_len = offset + self.c_addr().len();
        assert!(c_storage_len <= size_of::<libc::sockaddr_storage>());

        let c_storage_remain = unsafe {
            let ptr = (&mut c_storage as *mut _ as *mut u8).add(offset);
            let len = self.c_addr().len();
            std::slice::from_raw_parts_mut(ptr, len)
        };
        c_storage_remain.copy_from_slice(self.c_addr());
        (c_storage, c_storage_len)
    }
}

impl CSockAddr for libc::sockaddr_in {
    fn c_family(&self) -> libc::sa_family_t {
        libc::AF_INET as _
    }

    fn c_addr(&self) -> &[u8] {
        // Safety. The slice is part of self.
        unsafe {
            let addr_ptr = (self as *const _ as *const u8).add(size_of_val(&self.sin_family));
            std::slice::from_raw_parts(
                addr_ptr,
                size_of::<libc::sockaddr_in>() - size_of_val(&self.sin_family),
            )
        }
    }
}

impl CSockAddr for (libc::sockaddr_in, usize) {
    fn c_family(&self) -> libc::sa_family_t {
        self.0.c_family()
    }

    fn c_addr(&self) -> &[u8] {
        assert!(self.1 == size_of::<libc::sockaddr_in>());
        self.0.c_addr()
    }
}

impl CSockAddr for (libc::sockaddr_in6, usize) {
    fn c_family(&self) -> libc::sa_family_t {
        self.0.sin6_family
    }

    fn c_addr(&self) -> &[u8] {
        assert!(self.1 == size_of::<libc::sockaddr_in6>());
        unsafe {
            let addr_ptr = (&self.0 as *const _ as *const u8).add(size_of_val(&self.c_family()));
            std::slice::from_raw_parts(
                addr_ptr,
                size_of::<libc::sockaddr_in6>() - size_of_val(&self.c_family()),
            )
        }
    }
}

impl CSockAddr for (libc::sockaddr_un, usize) {
    fn c_family(&self) -> libc::sa_family_t {
        libc::AF_UNIX as _
    }

    fn c_addr(&self) -> &[u8] {
        assert!(
            size_of::<libc::sa_family_t>() <= self.1 && self.1 <= size_of::<libc::sockaddr_un>()
        );
        // Safety. The slice is part of self.
        unsafe {
            let addr_ptr = (&self.0 as *const _ as *const u8).add(size_of_val(&self.0.sun_family));
            std::slice::from_raw_parts(addr_ptr, self.1 - size_of_val(&self.0.sun_family))
        }
    }
}

impl CSockAddr for (libc::sockaddr_nl, usize) {
    fn c_family(&self) -> libc::sa_family_t {
        libc::AF_NETLINK as _
    }

    fn c_addr(&self) -> &[u8] {
        assert!(self.1 == size_of::<libc::sockaddr_nl>());

        unsafe {
            let addr_ptr = (&self.0 as *const _ as *const u8).add(size_of_val(&self.c_family()));
            std::slice::from_raw_parts(
                addr_ptr,
                size_of::<libc::sockaddr_nl>() - size_of_val(&self.c_family()),
            )
        }
    }
}

impl CSockAddr for (libc::sockaddr_storage, usize) {
    fn c_family(&self) -> libc::sa_family_t {
        self.0.ss_family
    }

    fn c_addr(&self) -> &[u8] {
        assert!(
            size_of::<libc::sa_family_t>() <= self.1
                && self.1 <= size_of::<libc::sockaddr_storage>()
        );
        // Safety. The slice is part of self.
        unsafe {
            let addr_ptr = (&self.0 as *const _ as *const u8).add(size_of_val(&self.0.ss_family));
            std::slice::from_raw_parts(addr_ptr, self.1 - size_of_val(&self.0.ss_family))
        }
    }
}
