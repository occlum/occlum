//! Attestation Device (/dev/attestation).

use super::*;

use util::mem_util::from_user::*;
use util::sgx::*;

use crate::fs::dev_fs::dev_sgx::SGX_DCAP_QUOTE_GENERATOR;

const ATTEST_REPORT_DATA_SIZE: size_t = 64;
// Big enough to save quote data
const ATTEST_QUOTE_SIZE: size_t = 5120;

#[derive(Debug)]
struct DevAttest {
    attest_type: String,
    report_data: [u8; ATTEST_REPORT_DATA_SIZE],
    quote: [u8; ATTEST_QUOTE_SIZE],
}

impl DevAttest {
    fn new() -> Self {
        Self {
            attest_type: "dcap".to_string(),
            report_data: [0; ATTEST_REPORT_DATA_SIZE],
            quote: [0; ATTEST_QUOTE_SIZE],
        }
    }
}

lazy_static! {
    static ref DEV_ATTEST: RwLock<DevAttest> = RwLock::new(DevAttest::new());
}

#[derive(Debug)]
pub struct DevAttestType;

impl INode for DevAttestType {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> vfs::Result<usize> {
        let dev_attest = DEV_ATTEST.read().unwrap();
        let attest_type = dev_attest.attest_type.clone();
        let len = attest_type.len();
        buf[0..len].copy_from_slice(attest_type.as_bytes());
        Ok(len)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> vfs::Result<usize> {
        Err(FsError::PermError)
    }

    fn poll(&self) -> vfs::Result<vfs::PollStatus> {
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
            mode: 0o444,
            nlinks: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
pub struct DevAttestReportData;

impl INode for DevAttestReportData {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> vfs::Result<usize> {
        if buf.len() < ATTEST_REPORT_DATA_SIZE {
            return Err(FsError::InvalidParam);
        }

        let dev_attest = DEV_ATTEST.read().unwrap();
        let report_data = &dev_attest.report_data;
        let len = report_data.len();
        buf[0..len].copy_from_slice(report_data);
        Ok(len)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> vfs::Result<usize> {
        if buf.len() > ATTEST_REPORT_DATA_SIZE {
            error!("Write buffer length is bigger than internal buffer size");
            return Err(FsError::InvalidParam);
        }

        let mut dev_attest = DEV_ATTEST.write().unwrap();
        let len = buf.len();
        dev_attest.report_data[0..len].copy_from_slice(buf);
        Ok(len)
    }

    fn poll(&self) -> vfs::Result<vfs::PollStatus> {
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

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
pub struct DevAttestQuote;

impl INode for DevAttestQuote {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> vfs::Result<usize> {
        if SGX_DCAP_QUOTE_GENERATOR.is_none() {
            error!("DCAP device not ready");
            return Err(FsError::NoDevice);
        }

        let quote_size = SGX_DCAP_QUOTE_GENERATOR.unwrap().get_quote_size() as usize;
        if buf.len() < quote_size {
            error!("Provided quote buffer is too small");
            return Err(FsError::InvalidParam);
        }
        trace!("quote size {}", quote_size);

        let dev_attest = DEV_ATTEST.read().unwrap();
        let mut report_data = sgx_report_data_t::default();

        //fill in the report data array
        report_data.d.clone_from(&dev_attest.report_data);

        let quote = SGX_DCAP_QUOTE_GENERATOR
            .unwrap()
            .generate_quote(&report_data)
            .map_err(|_| FsError::IOCTLError)?;

        buf[0..quote_size].copy_from_slice(&quote);

        Ok(quote_size)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> vfs::Result<usize> {
        Err(FsError::PermError)
    }

    fn poll(&self) -> vfs::Result<vfs::PollStatus> {
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
            mode: 0o444,
            nlinks: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}
