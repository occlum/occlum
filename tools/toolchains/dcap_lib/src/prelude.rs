pub use std::boxed::Box;
pub use libc::{open, ioctl, close, c_void, c_int, O_RDONLY};

// Defined in "occlum/deps/rust-sgx-sdk/sgx_types"
pub use sgx_types::{
    sgx_quote_header_t, sgx_report_data_t, sgx_ql_qv_result_t, sgx_report_body_t, sgx_quote3_t
};
