use std::cell::UnsafeCell;
#[cfg(feature = "sgx")]
use std::prelude::v1::*;

pub struct Page {
    // TODO: for SGX, this buffer needs to be allocated from a different source.
    buf: UnsafeCell<Vec<u8>>,
}

unsafe impl Send for Page {}
unsafe impl Sync for Page {}

impl Page {
    pub fn new() -> Self {
        let buf = UnsafeCell::new(Vec::with_capacity(Page::size()));
        Self { buf }
    }

    pub unsafe fn as_slice(&self) -> &[u8] {
        std::slice::from_raw_parts(self.as_ptr(), Self::size())
    }

    pub unsafe fn as_slice_mut(&self) -> &mut [u8] {
        std::slice::from_raw_parts_mut(self.as_mut_ptr(), Self::size())
    }

    pub fn as_ptr(&self) -> *const u8 {
        let buf = unsafe { &*self.buf.get() };
        buf.as_ptr()
    }

    pub fn as_mut_ptr(&self) -> *mut u8 {
        let buf = unsafe { &mut *self.buf.get() };
        buf.as_mut_ptr()
    }

    pub const fn size() -> usize {
        4096
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn fill_slice() {
        let page = Page::new();
        let slice_mut = unsafe { page.as_slice_mut() };
        slice_mut.fill(0xab);
        assert!(slice_mut.iter().all(|b| *b == 0xab));
    }
}
