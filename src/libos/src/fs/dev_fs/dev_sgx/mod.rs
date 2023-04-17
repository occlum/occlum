//! SGX Device (/dev/sgx).

use super::file_ops::{NonBuiltinIoctlCmd, StructuredIoctlArgType, StructuredIoctlNum};
use super::*;

mod consts;

use self::consts::*;
use util::mem_util::from_user::*;
use util::sgx::*;

extern "C" {
    static EDMM_supported: i32;
}

#[derive(Debug)]
pub struct DevSgx;

impl INode for DevSgx {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> vfs::Result<usize> {
        Err(FsError::PermError)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> vfs::Result<usize> {
        Err(FsError::PermError)
    }

    fn metadata(&self) -> vfs::Result<Metadata> {
        Ok(Metadata {
            dev: 1,
            inode: 0,
            size: 0,
            blk_size: 0,
            blocks: 0,
            atime: Timespec { sec: 0, nsec: 0 },
            mtime: Timespec { sec: 0, nsec: 0 },
            ctime: Timespec { sec: 0, nsec: 0 },
            type_: vfs::FileType::CharDevice,
            mode: 0o666,
            nlinks: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }

    fn io_control(&self, cmd: u32, data: usize) -> vfs::Result<()> {
        let mut ioctl_cmd = {
            let structured_cmd_num =
                StructuredIoctlNum::from_u32(cmd).map_err(|_| FsError::InvalidParam)?;
            let ioctl_cmd = unsafe {
                NonBuiltinIoctlCmd::new(structured_cmd_num, data as *mut u8)
                    .map_err(|_| FsError::InvalidParam)?
            };
            Box::new(ioctl_cmd)
        };
        self.ioctl(ioctl_cmd.as_mut()).map_err(|e| {
            error!("{}", e.backtrace());
            FsError::IOCTLError
        })?;
        Ok(())
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}

impl DevSgx {
    fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<i32> {
        let nonbuiltin_cmd = async_io::match_ioctl_cmd_mut!(cmd, {
            cmd : NonBuiltinIoctlCmd => cmd,
            _ => {
                return_errno!(EINVAL, "unsupported ioctl cmd for /dev/sgx");
            }
        });
        let cmd_num = nonbuiltin_cmd.cmd_num().as_u32();
        match cmd_num {
            SGX_CMD_NUM_IS_EDMM_SUPPORTED => {
                let arg = nonbuiltin_cmd.arg_mut::<i32>()?;
                *arg = unsafe { EDMM_supported };
            }
            SGX_CMD_NUM_GET_EPID_GROUP_ID => {
                let arg = nonbuiltin_cmd.arg_mut::<sgx_epid_group_id_t>()?;
                *arg = SGX_EPID_ATTEST_AGENT.lock().unwrap().get_epid_group_id()?;
            }
            SGX_CMD_NUM_GEN_EPID_QUOTE => {
                // Prepare the arguments
                let arg = nonbuiltin_cmd.arg_mut::<IoctlGenEPIDQuoteArg>()?;
                let sigrl = {
                    let sigrl_ptr = arg.sigrl_ptr;
                    let sigrl_len = arg.sigrl_len as usize;
                    if !sigrl_ptr.is_null() && sigrl_len > 0 {
                        let sigrl_slice =
                            unsafe { std::slice::from_raw_parts(sigrl_ptr, sigrl_len) };
                        Some(sigrl_slice)
                    } else {
                        None
                    }
                };
                let mut quote_output_buf = unsafe {
                    let quote_ptr = arg.quote_buf;
                    if quote_ptr.is_null() {
                        return_errno!(EINVAL, "the output buffer for quote cannot point to NULL");
                    }
                    let quote_len = arg.quote_buf_len as usize;
                    std::slice::from_raw_parts_mut(quote_ptr, quote_len)
                };

                // Generate the quote
                let quote = SGX_EPID_ATTEST_AGENT.lock().unwrap().generate_quote(
                    sigrl,
                    &arg.report_data,
                    arg.quote_type,
                    &arg.spid,
                    &arg.nonce,
                )?;
                quote.dump_to_buf(quote_output_buf)?;
            }
            SGX_CMD_NUM_SELF_TARGET => {
                let arg = nonbuiltin_cmd.arg_mut::<sgx_target_info_t>()?;
                *arg = get_self_target()?;
            }
            SGX_CMD_NUM_CREATE_REPORT => {
                // Prepare the arguments
                let arg = nonbuiltin_cmd.arg_mut::<IoctlCreateReportArg>()?;
                let target_info = if !arg.target_info.is_null() {
                    Some(unsafe { &*arg.target_info })
                } else {
                    None
                };
                let report_data = if !arg.report_data.is_null() {
                    Some(unsafe { &*arg.report_data })
                } else {
                    None
                };
                let report = {
                    if arg.report.is_null() {
                        return_errno!(EINVAL, "output pointer for report must not be null");
                    }
                    unsafe { &mut *arg.report }
                };
                *report = create_report(target_info, report_data)?;
            }
            SGX_CMD_NUM_VERIFY_REPORT => {
                let arg = nonbuiltin_cmd.arg::<sgx_report_t>()?;
                verify_report(arg)?;
            }
            SGX_CMD_NUM_DETECT_DCAP_DRIVER => {
                let arg = nonbuiltin_cmd.arg_mut::<i32>()?;
                unsafe {
                    let sgx_status = occlum_ocall_detect_dcap_driver(arg);
                    assert_eq!(sgx_status, sgx_status_t::SGX_SUCCESS);
                }

                extern "C" {
                    fn occlum_ocall_detect_dcap_driver(driver_installed: *mut i32) -> sgx_status_t;
                }
            }
            #[cfg(feature = "dcap")]
            SGX_CMD_NUM_GET_DCAP_QUOTE_SIZE => {
                let arg = nonbuiltin_cmd.arg_mut::<u32>()?;
                if SGX_DCAP_QUOTE_GENERATOR.is_none() {
                    return_errno!(EIO, "DCAP device not ready");
                }

                let quote_size = SGX_DCAP_QUOTE_GENERATOR.unwrap().get_quote_size();
                unsafe {
                    *arg = quote_size;
                }
            }
            #[cfg(feature = "dcap")]
            SGX_CMD_NUM_GEN_DCAP_QUOTE => {
                let arg = nonbuiltin_cmd.arg_mut::<IoctlGenDCAPQuoteArg>()?;
                check_ptr(arg.quote_size)?;
                let input_len = unsafe { *arg.quote_size };
                check_mut_array(arg.quote_buf, input_len as usize)?;

                if SGX_DCAP_QUOTE_GENERATOR.is_none() {
                    return_errno!(EIO, "DCAP device not ready");
                }

                let quote_size = SGX_DCAP_QUOTE_GENERATOR.unwrap().get_quote_size();
                if input_len < quote_size {
                    return_errno!(EINVAL, "provided quote is too small");
                }

                let quote = SGX_DCAP_QUOTE_GENERATOR
                    .unwrap()
                    .generate_quote(unsafe { &*arg.report_data })?;
                let mut input_quote_buf =
                    unsafe { std::slice::from_raw_parts_mut(arg.quote_buf, quote_size as usize) };
                input_quote_buf.copy_from_slice(&quote);
            }
            #[cfg(feature = "dcap")]
            SGX_CMD_NUM_GET_DCAP_SUPPLEMENTAL_SIZE => {
                let arg = nonbuiltin_cmd.arg_mut::<u32>()?;

                if SGX_DCAP_QUOTE_VERIFIER.is_none() {
                    return_errno!(EIO, "DCAP device not ready");
                }

                let supplemental_size = SGX_DCAP_QUOTE_VERIFIER
                    .unwrap()
                    .get_supplemental_data_size();
                unsafe {
                    *arg = supplemental_size;
                }
            }
            #[cfg(feature = "dcap")]
            SGX_CMD_NUM_VER_DCAP_QUOTE => {
                let arg = nonbuiltin_cmd.arg_mut::<IoctlVerDCAPQuoteArg>()?;
                let quote_size = arg.quote_size as usize;

                if SGX_DCAP_QUOTE_VERIFIER.is_none() {
                    return_errno!(EIO, "DCAP device not ready");
                }

                let supplemental_size = SGX_DCAP_QUOTE_VERIFIER
                    .unwrap()
                    .get_supplemental_data_size();
                check_array(arg.quote_buf, quote_size)?;
                let supplemental_slice = if !arg.supplemental_data.is_null() {
                    check_array(arg.supplemental_data, arg.supplemental_data_size as usize)?;
                    if arg.supplemental_data_size < supplemental_size {
                        return_errno!(EINVAL, "provided supplemental buffer is too short");
                    }

                    Some(unsafe {
                        std::slice::from_raw_parts_mut(
                            arg.supplemental_data,
                            supplemental_size as usize,
                        )
                    })
                } else {
                    None
                };

                let input_quote_buf =
                    unsafe { std::slice::from_raw_parts(arg.quote_buf, quote_size) };
                let (collateral_expiration_status, quote_verification_result, supplemental_data) =
                    SGX_DCAP_QUOTE_VERIFIER
                        .unwrap()
                        .verify_quote(input_quote_buf)?;

                unsafe {
                    *arg.collateral_expiration_status = collateral_expiration_status;
                    *arg.quote_verification_result = quote_verification_result;
                }

                if let Some(slice) = supplemental_slice {
                    slice.copy_from_slice(&supplemental_data);
                }
            }
            SGX_CMD_NUM_KEY => {
                // Prepare the arguments
                let arg = nonbuiltin_cmd.arg_mut::<IoctlGetKeyArg>()?;
                let key_request = {
                    if arg.key_request.is_null() {
                        return_errno!(EINVAL, "key_request must not be null");
                    }
                    unsafe { &*arg.key_request }
                };

                let key = {
                    if arg.key.is_null() {
                        return_errno!(EINVAL, "output pointer for key must not be null");
                    }
                    unsafe { &mut *arg.key }
                };
                *key = get_key(key_request)?;
            }
            _ => {
                return_errno!(ENOSYS, "unknown ioctl cmd for /dev/sgx");
            }
        }
        Ok(0)
    }
}

lazy_static! {
    pub static ref SGX_EPID_ATTEST_AGENT: SgxMutex<SgxEPIDAttestationAgent> =
        { SgxMutex::new(SgxEPIDAttestationAgent::new()) };
}

#[cfg(feature = "dcap")]
lazy_static! {
    pub static ref SGX_DCAP_QUOTE_GENERATOR: Option<SgxDCAPQuoteGenerator> =
        { SgxDCAPQuoteGenerator::new() };
    pub static ref SGX_DCAP_QUOTE_VERIFIER: Option<SgxDCAPQuoteVerifier> =
        { SgxDCAPQuoteVerifier::new() };
}

#[repr(C)]
struct IoctlGenEPIDQuoteArg {
    report_data: sgx_report_data_t,    // Input
    quote_type: sgx_quote_sign_type_t, // Input
    spid: sgx_spid_t,                  // Input
    nonce: sgx_quote_nonce_t,          // Input
    sigrl_ptr: *const u8,              // Input (optional)
    sigrl_len: u32,                    // Input (optional)
    quote_buf_len: u32,                // Input
    quote_buf: *mut u8,                // Output
}

#[repr(C)]
struct IoctlCreateReportArg {
    target_info: *const sgx_target_info_t, // Input (optional)
    report_data: *const sgx_report_data_t, // Input (optional)
    report: *mut sgx_report_t,             // Output
}

#[cfg(feature = "dcap")]
#[repr(C)]
struct IoctlGenDCAPQuoteArg {
    report_data: *const sgx_report_data_t, // Input
    quote_size: *mut u32,                  // Input/output
    quote_buf: *mut u8,                    // Output
}

#[cfg(feature = "dcap")]
#[repr(C)]
struct IoctlVerDCAPQuoteArg {
    quote_buf: *const u8,                               // Input
    quote_size: u32,                                    // Input
    collateral_expiration_status: *mut u32,             // Output
    quote_verification_result: *mut sgx_ql_qv_result_t, // Output
    supplemental_data_size: u32,                        // Input (optional)
    supplemental_data: *mut u8,                         // Output (optional)
}

#[repr(C)]
struct IoctlGetKeyArg {
    key_request: *const sgx_key_request_t, // Input
    key: *mut sgx_key_128bit_t,            // Output
}
