use std::boxed::Box;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sgxfs::{remove, OpenOptions, SgxFile};
use std::sync::{Arc, SgxMutex as Mutex};
use std::untrusted::fs;

use rcore_fs::dev::{DevError, DevResult};
use rcore_fs_sefs::dev::{File, SefsMac, Storage};

use super::*;
use crate::prelude::*;

/// A helper macro to automatically convert a block of code that returns `std::result::Result<T, E1>`
/// to one that returns `std::result::Result<T, E2>`, where `E1` satisfies `impl From<E1> for Error`
/// and `E2` satisfies `impl From<Error> for E2`.
///
/// This macro is designed to workaround the limitation that `From<E1>` cannot be implemented for
/// `E2` when both `E1` and `E2` are foreign types. For example, when `E1` is `std::io::Error` and
/// `E2` is `rcore_fs::dev::DevError`.
macro_rules! convert_result {
    ($body: block) => {{
        let mut closure_fn = || -> Result<_> { $body };
        Ok(closure_fn()?)
    }};
}

pub struct SgxStorage {
    path: PathBuf,
    integrity_only: bool,
    file_cache: Mutex<BTreeMap<u64, LockedFile>>,
    root_mac: Option<sgx_aes_gcm_128bit_tag_t>,
}

impl SgxStorage {
    pub fn new(
        path: impl AsRef<Path>,
        integrity_only: bool,
        file_mac: Option<sgx_aes_gcm_128bit_tag_t>,
    ) -> Self {
        // assert!(path.as_ref().is_dir());
        SgxStorage {
            path: path.as_ref().to_path_buf(),
            integrity_only: integrity_only,
            file_cache: Mutex::new(BTreeMap::new()),
            root_mac: file_mac,
        }
    }
    /// Get file by `file_id`.
    /// It lookups cache first, if miss, then call `open_fn` to open one,
    /// and add it to cache before return.
    #[cfg(feature = "sgx_file_cache")]
    fn get(
        &self,
        file_id: &str,
        open_fn: impl FnOnce(&Self) -> Result<LockedFile>,
    ) -> Result<LockedFile> {
        // query cache
        let key = self.calculate_hash(file_id);
        let mut caches = self.file_cache.lock().unwrap();
        if let Some(locked_file) = caches.get(&key) {
            // hit, return
            return Ok(locked_file.clone());
        }
        // miss, open one
        let locked_file = open_fn(self)?;
        // add to cache
        caches.insert(key, locked_file.clone());
        Ok(locked_file)
    }

    fn calculate_hash(&self, t: &str) -> u64 {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }
    /// Get file by `file_id` without cache.
    #[cfg(not(feature = "sgx_file_cache"))]
    fn get(
        &self,
        file_id: &str,
        open_fn: impl FnOnce(&Self) -> Result<LockedFile>,
    ) -> Result<LockedFile> {
        open_fn(self)
    }

    /// Set the expected root MAC of the SGX storage.
    ///
    /// By giving this root MAC, we can be sure that the root file (file_id = 0) opened
    /// by the storage has a MAC that is equal to the given root MAC.
    pub fn set_root_mac(&mut self, mac: sgx_aes_gcm_128bit_tag_t) -> Result<()> {
        self.root_mac = Some(mac);
        Ok(())
    }
}

impl Storage for SgxStorage {
    fn open(&self, file_id: &str) -> DevResult<Box<dyn File>> {
        let locked_file = self.get(file_id, |this| {
            let mut path = this.path.to_path_buf();
            path.push(file_id);
            let options = {
                let mut options = OpenOptions::new();
                options.read(true).update(true);
                options
            };
            let file = if !self.integrity_only {
                options.open(path)?
            } else {
                options.open_integrity_only(path)?
            };

            // Check the MAC of the root file against the given root MAC of the storage
            if file_id == "metadata" && self.root_mac.is_some() {
                let root_file_mac = file.get_mac().expect("Failed to get mac");
                if root_file_mac != self.root_mac.unwrap() {
                    error!(
                        "MAC validation for metadata file failed: expected = {:#?}, found = {:?}",
                        self.root_mac.unwrap(),
                        root_file_mac
                    );
                    return_errno!(EACCES);
                }
            }

            Ok(LockedFile(Arc::new(Mutex::new(file))))
        })?;
        Ok(Box::new(locked_file))
    }

    fn create(&self, file_id: &str) -> DevResult<Box<dyn File>> {
        let locked_file = self.get(file_id, |this| {
            let mut path = this.path.to_path_buf();
            path.push(file_id);
            let options = {
                let mut options = OpenOptions::new();
                options.write(true).update(true);
                options
            };
            let file = if !self.integrity_only {
                options.open(path)?
            } else {
                options.open_integrity_only(path)?
            };
            Ok(LockedFile(Arc::new(Mutex::new(file))))
        })?;
        Ok(Box::new(locked_file))
    }

    fn remove(&self, file_id: &str) -> DevResult<()> {
        convert_result!({
            let mut path = self.path.to_path_buf();
            path.push(file_id);
            remove(path)?;
            // remove from cache
            let key = self.calculate_hash(file_id);
            let mut caches = self.file_cache.lock().unwrap();
            caches.remove(&key);
            Ok(())
        })
    }

    fn is_integrity_only(&self) -> bool {
        self.integrity_only
    }

    fn clear(&self) -> DevResult<()> {
        convert_result!({
            for child in fs::read_dir(&self.path)? {
                let child = child?;
                remove(&child.path())?;
            }
            // clear cache
            let mut caches = self.file_cache.lock().unwrap();
            caches.clear();
            Ok(())
        })
    }
}

#[derive(Clone)]
pub struct LockedFile(Arc<Mutex<SgxFile>>);

// `sgx_tstd::sgxfs::SgxFile` not impl Send ...
unsafe impl Send for LockedFile {}
unsafe impl Sync for LockedFile {}

impl File for LockedFile {
    fn read_at(&self, buf: &mut [u8], offset: usize) -> DevResult<usize> {
        convert_result!({
            if buf.len() == 0 {
                return Ok(0);
            }
            let mut file = self.0.lock().unwrap();

            // SgxFile does not support to seek a position beyond the end.
            // So check if file_size < offset and return zero(indicates end of file).
            let file_size = file.seek(SeekFrom::End(0))? as usize;
            if file_size < offset {
                return Ok(0);
            }

            let offset = offset as u64;
            file.seek(SeekFrom::Start(offset))?;
            let len = file.read(buf)?;
            Ok(len)
        })
    }

    fn write_at(&self, buf: &[u8], offset: usize) -> DevResult<usize> {
        convert_result!({
            if buf.len() == 0 {
                return Ok(0);
            }
            let mut file = self.0.lock().unwrap();

            // SgxFile does not support to seek a position beyond the end.
            // So check if file_size < offset and padding null bytes.
            let file_size = file.seek(SeekFrom::End(0))? as usize;
            if file_size < offset {
                static ZEROS: [u8; 0x1000] = [0; 0x1000];
                let mut rest_len = offset - file_size;
                while rest_len != 0 {
                    let l = rest_len.min(0x1000);
                    let len = file.write(&ZEROS[..l])?;
                    rest_len -= len;
                }
            }

            let offset = offset as u64;
            file.seek(SeekFrom::Start(offset))?;
            let len = file.write(buf)?;
            Ok(len)
        })
    }

    fn set_len(&self, len: usize) -> DevResult<()> {
        // NOTE: do nothing ??
        Ok(())
    }

    fn flush(&self) -> DevResult<()> {
        convert_result!({
            let mut file = self.0.lock().unwrap();
            file.flush()?;
            Ok(())
        })
    }

    fn get_file_mac(&self) -> DevResult<SefsMac> {
        let file = self.0.lock().unwrap();
        Ok(SefsMac(file.get_mac().unwrap()))
    }
}
