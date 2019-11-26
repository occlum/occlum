//! SGX attestation.

use super::*;

use sgx_tse::*;
use sgx_types::*;

mod sgx_attestation_agent;
mod sgx_quote;

pub use sgx_types::{
    sgx_epid_group_id_t, sgx_quote_nonce_t, sgx_quote_sign_type_t, sgx_quote_t, sgx_report_data_t,
    sgx_spid_t, sgx_target_info_t,
};

pub use self::sgx_attestation_agent::SgxAttestationAgent;
pub use self::sgx_quote::SgxQuote;
