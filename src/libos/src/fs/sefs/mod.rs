use super::sgx_aes_gcm_128bit_tag_t;

pub use self::sgx_storage::SgxStorage;
pub use self::sgx_uuid_provider::SgxUuidProvider;

mod sgx_storage;
mod sgx_uuid_provider;
