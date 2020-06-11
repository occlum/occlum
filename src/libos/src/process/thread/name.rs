use crate::prelude::*;

use std::ffi::{CStr, CString};

// The thread name buffer should allow space for up to 16 bytes, including the terminating null byte.
const THREAD_NAME_MAX_LEN: usize = 16;

/// A thread name represented in a fixed buffer of 16 bytes.
///
/// The length is chosen to be consistent with Linux.
#[derive(Debug, Clone, Default)]
pub struct ThreadName {
    buf: [u8; THREAD_NAME_MAX_LEN],
    len: usize, // including null terminator
}

impl ThreadName {
    /// Construct a thread name from str
    pub fn new(name: &str) -> Self {
        Self::from_slice(CString::new(name).unwrap().as_bytes_with_nul())
    }

    pub const fn max_len() -> usize {
        THREAD_NAME_MAX_LEN
    }

    /// Construct a thread name from slice
    pub fn from_slice(input: &[u8]) -> Self {
        let mut buf = [0; THREAD_NAME_MAX_LEN];
        let mut len = THREAD_NAME_MAX_LEN;
        for (i, b) in buf.iter_mut().take(THREAD_NAME_MAX_LEN - 1).enumerate() {
            if input[i] == '\0' as u8 {
                len = i + 1;
                break;
            }
            *b = input[i];
        }
        debug_assert!(buf[THREAD_NAME_MAX_LEN - 1] == 0);
        Self { buf, len }
    }

    /// Returns a byte slice
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    /// Converts to a CStr.
    pub fn as_c_str(&self) -> &CStr {
        // Note: from_bytes_with_nul will fail if slice has more than 1 '\0' at the end
        CStr::from_bytes_with_nul(&self.buf[..self.len]).unwrap_or_default()
    }
}
