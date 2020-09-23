use super::*;
use std::*;

#[derive(Copy, Clone)]
pub struct SockAddr {
    storage: libc::sockaddr_storage,
    len: usize,
}

// TODO: add more fields
impl fmt::Debug for SockAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SockAddr")
            .field(
                "family",
                &AddressFamily::try_from(self.storage.ss_family).unwrap(),
            )
            .field("len", &self.len)
            .finish()
    }
}

impl SockAddr {
    // Caller should guarentee the sockaddr and addr_len are valid
    pub unsafe fn try_from_raw(
        sockaddr: *const libc::sockaddr,
        addr_len: libc::socklen_t,
    ) -> Result<Self> {
        if addr_len < std::mem::size_of::<libc::sa_family_t>() as u32 {
            return_errno!(EINVAL, "the address is too short.");
        }

        if addr_len > std::mem::size_of::<libc::sockaddr_storage>() as u32 {
            return_errno!(EINVAL, "the address is too long.");
        }

        match AddressFamily::try_from((*sockaddr).sa_family)? {
            AddressFamily::INET => {
                if addr_len < std::mem::size_of::<libc::sockaddr_in>() as u32 {
                    return_errno!(EINVAL, "short ipv4 address.");
                }
            }
            AddressFamily::INET6 => {
                let ipv6_addr_len = std::mem::size_of::<libc::sockaddr_in6>() as u32;
                // Omit sin6_scope_id when it is not fully provided
                // 4 represents the size of sin6_scope_id which is not a must
                if addr_len < ipv6_addr_len - 4 {
                    return_errno!(EINVAL, "wrong ipv6 address length.");
                }
            }
            _ => warn!("address family not checked"),
        }

        let mut storage = mem::MaybeUninit::<libc::sockaddr_storage>::uninit();
        ptr::copy_nonoverlapping(
            sockaddr as *const _ as *const u8,
            storage.as_mut_ptr() as *mut u8,
            addr_len as usize,
        );
        Ok(Self {
            storage: storage.assume_init(),
            len: addr_len as usize,
        })
    }

    pub fn as_ptr_and_len(&self) -> (*const libc::sockaddr, usize) {
        (self.as_ptr(), self.len())
    }

    pub fn as_ptr(&self) -> *const libc::sockaddr {
        &self.storage as *const _ as *const _
    }

    pub fn as_mut_ptr(&mut self) -> *mut libc::sockaddr {
        &mut self.storage as *mut _ as *mut _
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len()) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.as_mut_ptr() as *mut u8, self.len()) }
    }

    pub fn copy_to_slice(&self, dst: &mut [u8]) -> usize {
        let (addr_ptr, addr_len) = self.as_ptr_and_len();
        let copy_len = std::cmp::min(addr_len, dst.len());
        dst[0..copy_len].copy_from_slice(unsafe {
            std::slice::from_raw_parts(addr_ptr as *const u8, copy_len)
        });
        copy_len
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn set_len(&mut self, len: usize) -> Result<()> {
        if len > Self::capacity() {
            return_errno!(EINVAL, "length is too long")
        } else {
            self.len = len;
            Ok(())
        }
    }

    pub fn capacity() -> usize {
        mem::size_of::<libc::sockaddr_storage>()
    }
}

impl Default for SockAddr {
    fn default() -> Self {
        let mut storage: libc::sockaddr_storage = unsafe { mem::zeroed() };
        Self {
            storage: storage,
            len: mem::size_of::<libc::sockaddr_storage>(),
        }
    }
}
