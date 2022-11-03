use super::{sgx_aes_gcm_128bit_tag_t, sgx_key_128bit_t};

pub use self::sgx_storage::SgxStorage;
pub use self::sgx_uuid_provider::SgxUuidProvider;

mod sgx_storage;
mod sgx_uuid_provider;

// Cache size of underlying SGX-PFS of SEFS
// Default cache size: 0x1000 * 48
const SEFS_CACHE_SIZE: u64 = 0x1000 * 256;
