//! I/O vectors

use super::*;

/// A memory safe, immutable version of C iovec array
pub struct Iovs<'a> {
    iovs: Vec<&'a [u8]>,
}

impl<'a> Iovs<'a> {
    pub fn new(slices: Vec<&'a [u8]>) -> Iovs {
        Self { iovs: slices }
    }

    pub fn as_slices(&self) -> &[&[u8]] {
        &self.iovs[..]
    }

    pub fn total_bytes(&self) -> usize {
        self.iovs.iter().map(|s| s.len()).sum()
    }

    pub fn gather_to_vec(&self) -> Vec<u8> {
        Self::gather_slices_to_vec(&self.iovs[..])
    }

    fn gather_slices_to_vec(slices: &[&[u8]]) -> Vec<u8> {
        let vec_len = slices.iter().map(|slice| slice.len()).sum();
        let mut vec = Vec::with_capacity(vec_len);
        for slice in slices {
            vec.extend_from_slice(slice);
        }
        vec
    }
}

/// A memory safe, mutable version of C iovec array
pub struct IovsMut<'a> {
    iovs: Vec<&'a mut [u8]>,
}

impl<'a> IovsMut<'a> {
    pub fn new(slices: Vec<&'a mut [u8]>) -> Self {
        Self { iovs: slices }
    }

    pub fn as_slices<'b>(&'b self) -> &'b [&'a [u8]] {
        let slices_mut: &'b [&'a mut [u8]] = &self.iovs[..];
        // We are "downgrading" _mutable_ slices to _immutable_ ones. It should be
        // safe to do this transmute
        unsafe { std::mem::transmute(slices_mut) }
    }

    pub fn as_slices_mut<'b>(&'b mut self) -> &'b mut [&'a mut [u8]] {
        &mut self.iovs[..]
    }

    pub fn total_bytes(&self) -> usize {
        self.iovs.iter().map(|s| s.len()).sum()
    }

    pub fn gather_to_vec(&self) -> Vec<u8> {
        Iovs::gather_slices_to_vec(self.as_slices())
    }

    pub fn scatter_copy_from(&mut self, data: &[u8]) -> usize {
        let mut total_nbytes = 0;
        let mut remain_slice = data;
        for iov in &mut self.iovs {
            if remain_slice.len() == 0 {
                break;
            }

            let copy_nbytes = remain_slice.len().min(iov.len());
            let dst_slice = unsafe {
                debug_assert!(iov.len() >= copy_nbytes);
                iov.get_unchecked_mut(..copy_nbytes)
            };
            let (src_slice, _remain_slice) = remain_slice.split_at(copy_nbytes);
            dst_slice.copy_from_slice(src_slice);

            remain_slice = _remain_slice;
            total_nbytes += copy_nbytes;
        }
        debug_assert!(remain_slice.len() == 0);
        total_nbytes
    }
}
