use super::*;
use crate::fs::KeyPolicy;
use crate::misc::get_random;

use std::io::{Read, Write};
use std::mem::{size_of, transmute};
use std::path::PathBuf;
use std::ptr;
use std::untrusted::{fs::File, path::PathEx};

pub fn get_key(key_request: &sgx_key_request_t) -> Result<sgx_key_128bit_t> {
    let mut key = sgx_key_128bit_t::default();
    let sgx_status = unsafe { sgx_get_key(key_request, &mut key as *mut sgx_key_128bit_t) };
    match sgx_status {
        sgx_status_t::SGX_SUCCESS => Ok(key),
        sgx_status_t::SGX_ERROR_INVALID_PARAMETER => return_errno!(EINVAL, "invalid paramters"),
        _ => {
            error!("sgx_get_key return {:?}", sgx_status);
            return_errno!(EINVAL, "unexpected SGX error")
        }
    }
}

pub fn get_autokey_with_policy(
    autokey_policy: &Option<u32>,
    path: &PathBuf,
) -> Result<sgx_key_128bit_t> {
    let mut key_request = sgx_key_request_t::default();

    key_request.key_name = SGX_KEYSELECT_SEAL;
    key_request.key_policy = {
        let key_policy = autokey_policy
            .and_then(|policy| KeyPolicy::from_u32(policy))
            .map_or(SGX_KEYPOLICY_MRSIGNER, |policy| policy.bits());
        if (key_policy
            & !(SGX_KEYPOLICY_MRENCLAVE
                | SGX_KEYPOLICY_MRSIGNER
                | SGX_KEYPOLICY_CONFIGID
                | SGX_KEYPOLICY_ISVFAMILYID
                | SGX_KEYPOLICY_ISVEXTPRODID
                | SGX_KEYPOLICY_NOISVPRODID)
            != 0)
            || (key_policy & (SGX_KEYPOLICY_MRENCLAVE | SGX_KEYPOLICY_MRSIGNER)) == 0
        {
            return_errno!(EINVAL, "autokey policy is invalid")
        }
        key_policy
    };

    let key_metadata = {
        let metadata_path = {
            let mut metadata_path = path.clone();
            if !metadata_path.is_dir() {
                metadata_path = metadata_path.parent().unwrap().to_path_buf();
            }
            metadata_path.push(KEY_METADATA_FILE_NAME);
            metadata_path
        };
        let fetch_res = if metadata_path.exists() {
            KeyMetadata::fetch_from(&metadata_path)
        } else {
            Err(errno!(ENOENT))
        };
        fetch_res.unwrap_or_else(|_| {
            let key_metadata = KeyMetadata::default();
            let _ = key_metadata.persist_to(&metadata_path);
            key_metadata
        })
    };
    key_request.key_id = key_metadata.key_id;
    key_request.cpu_svn = key_metadata.cpu_svn;
    key_request.isv_svn = key_metadata.isv_svn;

    key_request.attribute_mask.flags = TSEAL_DEFAULT_FLAGSMASK;
    key_request.attribute_mask.xfrm = 0x0;
    key_request.misc_mask = TSEAL_DEFAULT_MISCMASK;

    get_key(&key_request)
}

const KEY_METADATA_FILE_NAME: &str = "metadata";
const KEY_METADATA_SIZE: usize = size_of::<KeyMetadata>();

#[repr(C)]
struct KeyMetadata {
    pub key_id: sgx_key_id_t,
    pub cpu_svn: sgx_cpu_svn_t,
    pub isv_svn: sgx_isv_svn_t,
}

impl Default for KeyMetadata {
    fn default() -> Self {
        let key_id = {
            let mut key_id = sgx_key_id_t::default();
            let _ = get_random(&mut key_id.id);
            key_id
        };
        let report = sgx_tse::rsgx_self_report();
        Self {
            key_id,
            cpu_svn: report.body.cpu_svn,
            isv_svn: report.body.isv_svn,
        }
    }
}

impl KeyMetadata {
    fn fetch_from(metadata_path: &PathBuf) -> Result<Self> {
        debug_assert!(metadata_path.ends_with(KEY_METADATA_FILE_NAME));
        let mut metadata_file = File::open(metadata_path)?;
        let mut metadata_buf = [0u8; KEY_METADATA_SIZE];
        metadata_file.read(&mut metadata_buf)?;
        Ok(Self::from_bytes(metadata_buf))
    }

    fn persist_to(&self, metadata_path: &PathBuf) -> Result<()> {
        debug_assert!(metadata_path.ends_with(KEY_METADATA_FILE_NAME));
        let mut metadata_file = File::create(metadata_path)?;
        metadata_file.write(self.to_bytes())?;
        Ok(())
    }

    fn to_bytes(&self) -> &[u8; KEY_METADATA_SIZE] {
        unsafe { transmute::<&Self, &[u8; KEY_METADATA_SIZE]>(self) }
    }

    fn from_bytes(bytes: [u8; KEY_METADATA_SIZE]) -> Self {
        unsafe { transmute::<[u8; KEY_METADATA_SIZE], Self>(bytes) }
    }
}
