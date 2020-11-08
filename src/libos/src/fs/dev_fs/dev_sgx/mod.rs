//! SGX Device (/dev/sgx).

use super::*;

mod consts;

use self::consts::*;
use util::sgx::*;

#[derive(Debug)]
pub struct DevSgx;

impl File for DevSgx {
    fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<i32> {
        let nonbuiltin_cmd = match cmd {
            IoctlCmd::NonBuiltin(nonbuiltin_cmd) => nonbuiltin_cmd,
            _ => return_errno!(EINVAL, "unknown ioctl cmd for /dev/sgx"),
        };
        let cmd_num = nonbuiltin_cmd.cmd_num().as_u32();
        match cmd_num {
            SGX_CMD_NUM_IS_EDMM_SUPPORTED => {
                let arg = nonbuiltin_cmd.arg_mut::<i32>()?;
                *arg = 0; // no support for now
            }
            SGX_CMD_NUM_GET_EPID_GROUP_ID => {
                let arg = nonbuiltin_cmd.arg_mut::<sgx_epid_group_id_t>()?;
                *arg = SGX_ATTEST_AGENT.lock().unwrap().get_epid_group_id()?;
            }
            SGX_CMD_NUM_GEN_QUOTE => {
                // Prepare the arguments
                let arg = nonbuiltin_cmd.arg_mut::<IoctlGenQuoteArg>()?;
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
                let quote = SGX_ATTEST_AGENT.lock().unwrap().generate_quote(
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
            _ => {
                return_errno!(ENOSYS, "unknown ioctl cmd for /dev/sgx");
            }
        }
        Ok(0)
    }

    fn poll_new(&self) -> IoEvents {
        IoEvents::IN
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

lazy_static! {
    /// The root of file system
    pub static ref SGX_ATTEST_AGENT: SgxMutex<SgxAttestationAgent> = {
        SgxMutex::new(SgxAttestationAgent::new())
    };
}

#[repr(C)]
struct IoctlGenQuoteArg {
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
