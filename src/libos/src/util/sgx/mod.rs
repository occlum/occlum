//! SGX utility.

use super::*;

use sgx_tse::*;
use sgx_types::*;

#[cfg(feature = "dcap")]
mod dcap;
mod epid;
mod sgx_key;
mod sgx_report;

pub use sgx_types::{
    sgx_create_report, sgx_epid_group_id_t, sgx_quote_nonce_t, sgx_quote_sign_type_t, sgx_quote_t,
    sgx_report_data_t, sgx_self_target, sgx_spid_t, sgx_target_info_t, sgx_verify_report,
};

#[cfg(feature = "dcap")]
pub use self::dcap::{
    QuoteGenerator as SgxDCAPQuoteGenerator, QuoteVerifier as SgxDCAPQuoteVerifier,
};
pub use self::epid::AttestationAgent as SgxEPIDAttestationAgent;
pub use self::sgx_key::{get_autokey_with_policy, get_key};
pub use self::sgx_report::{create_report, get_self_target, verify_report};

pub fn allow_debug() -> bool {
    let self_report = create_report(None, None).expect("create a self report should never fail");
    (self_report.body.attributes.flags & SGX_FLAGS_DEBUG) == SGX_FLAGS_DEBUG
}
