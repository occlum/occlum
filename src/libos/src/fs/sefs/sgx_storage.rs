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
    encrypt_mode: EncryptMode,
    file_cache: Mutex<BTreeMap<u64, LockedFile>>,
    cache_size: Option<u64>,
}

impl SgxStorage {
    pub fn new(
        path: impl AsRef<Path>,
        key: &Option<sgx_key_128bit_t>,
        root_mac: &Option<sgx_aes_gcm_128bit_tag_t>,
        autokey_policy: &Option<u32>,
        cache_size: Option<u64>,
    ) -> Result<Self> {
        // assert!(path.as_ref().is_dir());
        Self::check_cache_size(&cache_size)?;
        Ok(SgxStorage {
            path: path.as_ref().to_path_buf(),
            encrypt_mode: EncryptMode::new(key, root_mac, autokey_policy),
            file_cache: Mutex::new(BTreeMap::new()),
            cache_size,
        })
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

    fn check_cache_size(cache_size: &Option<u64>) -> Result<()> {
        const PAGE_SIZE: u64 = 0x1000;
        const DEFAULT_CACHE_SIZE: u64 = 48 * PAGE_SIZE;
        if let Some(size) = *cache_size {
            if size < DEFAULT_CACHE_SIZE || size % PAGE_SIZE != 0 {
                error!(
                    "invalid cache size: {}, must larger than default size: {} and aligned with page size: {}",
                    size, DEFAULT_CACHE_SIZE, PAGE_SIZE
                );
                return_errno!(EINVAL, "invalid cache size");
            }
        }
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
            let file = match self.encrypt_mode {
                EncryptMode::IntegrityOnly(_) => options.open_integrity_only(path)?,
                EncryptMode::EncryptWithIntegrity(key, _) | EncryptMode::Encrypt(key) => {
                    options.open_with(path, Some(&key), None, self.cache_size)?
                }
                EncryptMode::EncryptAutoKey(key_policy) => match key_policy {
                    None => options.open_with(path, None, None, self.cache_size)?,
                    Some(policy) => {
                        options.open_with(path, None, Some(policy.bits()), self.cache_size)?
                    }
                },
            };

            // Check the MAC of the root file against the given root MAC of the storage
            if file_id == "metadata" && self.protect_integrity() {
                let root_file_mac = file.get_mac().expect("Failed to get mac");
                if root_file_mac != self.encrypt_mode.root_mac().unwrap() {
                    error!(
                        "MAC validation for metadata file failed: expected = {:#?}, found = {:?}",
                        self.encrypt_mode.root_mac().unwrap(),
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
            let file = match self.encrypt_mode {
                EncryptMode::IntegrityOnly(_) => options.open_integrity_only(path)?,
                EncryptMode::EncryptWithIntegrity(key, _) | EncryptMode::Encrypt(key) => {
                    options.open_with(path, Some(&key), None, self.cache_size)?
                }
                EncryptMode::EncryptAutoKey(key_policy) => match key_policy {
                    None => options.open_with(path, None, None, self.cache_size)?,
                    Some(policy) => {
                        options.open_with(path, None, Some(policy.bits()), self.cache_size)?
                    }
                },
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

    fn protect_integrity(&self) -> bool {
        match self.encrypt_mode {
            EncryptMode::IntegrityOnly(_) | EncryptMode::EncryptWithIntegrity(_, _) => true,
            _ => false,
        }
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

enum EncryptMode {
    IntegrityOnly(sgx_aes_gcm_128bit_tag_t),
    EncryptWithIntegrity(sgx_key_128bit_t, sgx_aes_gcm_128bit_tag_t),
    Encrypt(sgx_key_128bit_t),
    EncryptAutoKey(Option<KeyPolicy>),
}

impl EncryptMode {
    pub fn new(
        key: &Option<sgx_key_128bit_t>,
        root_mac: &Option<sgx_aes_gcm_128bit_tag_t>,
        autokey_policy: &Option<u32>,
    ) -> Self {
        match (key, root_mac) {
            (Some(key), Some(root_mac)) => Self::EncryptWithIntegrity(*key, *root_mac),
            (Some(key), None) => Self::Encrypt(*key),
            (None, Some(root_mac)) => Self::IntegrityOnly(*root_mac),
            (None, None) => {
                let policy = autokey_policy.and_then(|policy| KeyPolicy::from_u32(policy));
                Self::EncryptAutoKey(policy)
            }
        }
    }

    pub fn root_mac(&self) -> Option<sgx_aes_gcm_128bit_tag_t> {
        match self {
            Self::IntegrityOnly(root_mac) | Self::EncryptWithIntegrity(_, root_mac) => {
                Some(*root_mac)
            }
            _ => None,
        }
    }
}

bitflags! {
    pub struct KeyPolicy: u16 {
        const KEYPOLICY_MRENCLAVE = 0x0001;
        const KEYPOLICY_MRSIGNER = 0x0002;
        const KEYPOLICY_NOISVPRODID = 0x0004;
        const KEYPOLICY_CONFIGID = 0x0008;
        const KEYPOLICY_ISVFAMILYID = 0x0010;
        const KEYPOLICY_ISVEXTPRODID = 0x0020;
    }
}

impl KeyPolicy {
    pub fn from_u32(bits: u32) -> Option<Self> {
        // If the bits is not zero, set the key_policy to "MRSIGNER|MRENCLAVE|ISVFAMILYID"
        if bits != 0 {
            let key_policy =
                Self::KEYPOLICY_MRSIGNER | Self::KEYPOLICY_MRENCLAVE | Self::KEYPOLICY_ISVFAMILYID;
            Some(key_policy)
        } else {
            None
        }
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
                let mut remaining_len = offset - file_size;
                while remaining_len != 0 {
                    let l = remaining_len.min(0x1000);
                    let len = file.write(&ZEROS[..l])?;
                    remaining_len -= len;
                }
            }

            let offset = offset as u64;
            file.seek(SeekFrom::Start(offset))?;
            let len = file.write(buf)?;
            Ok(len)
        })
    }

    fn set_len(&self, len: usize) -> DevResult<()> {
        // The set_len() is unsupported for SgxFile, we have to
        // implement it in a slow way by padding null bytes.
        convert_result!({
            let mut file = self.0.lock().unwrap();
            let file_size = file.seek(SeekFrom::End(0))? as usize;
            let mut reset_len = if len > file_size {
                // Expand the file by padding null bytes
                len - file_size
            } else {
                // Shrink the file by setting null bytes between len and file_size
                file.seek(SeekFrom::Start(len as u64))?;
                file_size - len
            };
            static ZEROS: [u8; 0x1000] = [0; 0x1000];
            while reset_len != 0 {
                let l = reset_len.min(0x1000);
                // Probably there's not enough space on disk, let's panic here
                let written_len = file.write(&ZEROS[..l]).unwrap_or_else(|e| {
                    error!("failed to set null bytes: {}", e);
                    panic!();
                });
                reset_len -= written_len;
            }
            Ok(())
        })
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
