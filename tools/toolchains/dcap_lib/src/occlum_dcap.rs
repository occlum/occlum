use crate::prelude::*;
use std::ffi::CString;

const SGXIOC_GET_DCAP_QUOTE_SIZE: u64 = 0x80047307;
const SGXIOC_GEN_DCAP_QUOTE: u64 = 0xc0187308;
const SGXIOC_GET_DCAP_SUPPLEMENTAL_SIZE: u64 = 0x80047309;
const SGXIOC_VER_DCAP_QUOTE: u64 = 0xc030730a;

cfg_if::cfg_if! {
    if #[cfg(target_env = "musl")] {
        const IOCTL_GET_DCAP_QUOTE_SIZE: i32 = SGXIOC_GET_DCAP_QUOTE_SIZE as i32;
        const IOCTL_GEN_DCAP_QUOTE: i32 = SGXIOC_GEN_DCAP_QUOTE as i32;
        const IOCTL_GET_DCAP_SUPPLEMENTAL_SIZE: i32 = SGXIOC_GET_DCAP_SUPPLEMENTAL_SIZE as i32;
        const IOCTL_VER_DCAP_QUOTE: i32 = SGXIOC_VER_DCAP_QUOTE as i32;
    } else {
        const IOCTL_GET_DCAP_QUOTE_SIZE: u64 = SGXIOC_GET_DCAP_QUOTE_SIZE;
        const IOCTL_GEN_DCAP_QUOTE: u64 = SGXIOC_GEN_DCAP_QUOTE;
        const IOCTL_GET_DCAP_SUPPLEMENTAL_SIZE: u64 = SGXIOC_GET_DCAP_SUPPLEMENTAL_SIZE;
        const IOCTL_VER_DCAP_QUOTE: u64 = SGXIOC_VER_DCAP_QUOTE;
    }
}

// Copy from occlum/src/libos/src/fs/dev_fs/dev_sgx/mod.rs
//#[allow(dead_code)]
#[repr(C)]
pub struct IoctlGenDCAPQuoteArg {
    pub report_data: *const sgx_report_data_t, // Input
    pub quote_size: *mut u32,                  // Input/output
    pub quote_buf: *mut u8,                    // Output
}

// Copy from occlum/src/libos/src/fs/dev_fs/dev_sgx/mod.rs
//#[allow(dead_code)]
#[repr(C)]
pub struct IoctlVerDCAPQuoteArg {
    pub quote_buf: *const u8,                               // Input
    pub quote_size: u32,                                    // Input
    pub collateral_expiration_status: *mut u32,             // Output
    pub quote_verification_result: *mut sgx_ql_qv_result_t, // Output
    pub supplemental_data_size: u32,                        // Input (optional)
    pub supplemental_data: *mut u8,                         // Output (optional)
}

pub struct DcapQuote {
    fd: c_int,
    quote_size: u32,
    supplemental_size: u32,
}

impl DcapQuote {
    pub fn new() -> Result<Self, Error> {
        let path = CString::new("/dev/sgx").unwrap();
        let fd = unsafe { libc::open(path.as_ptr(), O_RDONLY) };
        if fd > 0 {
            Ok(Self {
                fd: fd,
                quote_size: 0,
                supplemental_size: 0,
            })
        } else {
            let os_err = Error::last_os_error();
            println!("OS error: {os_err:?}");
            Err(os_err)
        }
    }

    pub fn get_quote_size(&mut self) -> Result<u32, Error> {
        let size: u32 = 0;
        let ret = unsafe { libc::ioctl(self.fd, IOCTL_GET_DCAP_QUOTE_SIZE, &size) };
        if ret < 0 {
            let os_err = Error::last_os_error();
            println!("OS error: {os_err:?}");
            Err(os_err)
        } else {
            self.quote_size = size;
            Ok(size)
        }
    }

    pub fn generate_quote(
        &mut self,
        quote_buf: *mut u8,
        report_data: *const sgx_report_data_t,
    ) -> Result<i32, Error> {
        let quote_arg: IoctlGenDCAPQuoteArg = IoctlGenDCAPQuoteArg {
            report_data: report_data,
            quote_size: &mut self.quote_size,
            quote_buf: quote_buf,
        };

        let ret = unsafe { libc::ioctl(self.fd, IOCTL_GEN_DCAP_QUOTE, &quote_arg) };
        if ret < 0 {
            let os_err = Error::last_os_error();
            println!("OS error: {os_err:?}");
            Err(os_err)
        } else {
            Ok(0)
        }
    }

    pub fn get_supplemental_data_size(&mut self) -> Result<u32, Error> {
        let size: u32 = 0;
        let ret = unsafe { libc::ioctl(self.fd, IOCTL_GET_DCAP_SUPPLEMENTAL_SIZE, &size) };
        if ret < 0 {
            let os_err = Error::last_os_error();
            println!("OS error: {os_err:?}");
            Err(os_err)
        } else {
            self.supplemental_size = size;
            Ok(size)
        }
    }

    pub fn verify_quote(&mut self, verify_arg: *mut IoctlVerDCAPQuoteArg) -> Result<i32, Error> {
        let ret = unsafe { libc::ioctl(self.fd, IOCTL_VER_DCAP_QUOTE, verify_arg) };
        if ret < 0 {
            let os_err = Error::last_os_error();
            println!("OS error: {os_err:?}");
            Err(os_err)
        } else {
            Ok(0)
        }
    }

    pub fn close(&mut self) {
        unsafe { libc::close(self.fd) };
    }
}
