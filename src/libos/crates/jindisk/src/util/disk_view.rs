//! Disk view.
use crate::prelude::*;

use std::fmt::{self, Debug};

/// Disk view that manages underlying disk and
/// restrict read/write boundaries.
#[derive(Clone)]
pub struct DiskView {
    boundary: HbaRange,
    disk: Arc<dyn BlockDevice>,
}

impl DiskView {
    pub fn new(boundary: HbaRange, disk: Arc<dyn BlockDevice>) -> Self {
        debug_assert!(
            (boundary.end().to_raw() as usize) <= disk.total_blocks() && !boundary.is_empty(),
            "check boundary failed: {:?}",
            boundary
        );
        Self { boundary, disk }
    }

    pub fn submit(&self, req: Arc<BioReq>) -> Result<BioSubmission> {
        self.check_boundary(req.addr(), req.num_blocks() * BLOCK_SIZE)?;
        Ok(self.disk.submit(req))
    }

    pub async fn read(&self, addr: Hba, buf: &mut [u8]) -> Result<usize> {
        self.check_boundary(addr, buf.len())?;
        self.disk.read(addr.to_offset(), buf).await
    }

    pub async fn write(&self, addr: Hba, buf: &[u8]) -> Result<usize> {
        self.check_boundary(addr, buf.len())?;
        self.disk.write(addr.to_offset(), buf).await
    }

    pub async fn sync(&self) -> Result<()> {
        self.disk.sync().await
    }

    pub fn total_bytes(&self) -> usize {
        self.disk.total_bytes()
    }

    pub fn boundary(&self) -> &HbaRange {
        &self.boundary
    }

    fn check_boundary(&self, addr: Hba, buf_len: usize) -> Result<()> {
        debug_assert!(buf_len % BLOCK_SIZE == 0);
        let target_range =
            HbaRange::new(addr..addr + Hba::from_byte_offset_aligned(buf_len).unwrap().to_raw());
        if !self.boundary.is_sub_range(&target_range) {
            return_errno!(EINVAL, "read/write buffer not in legal boundary")
        }
        Ok(())
    }

    // Test-purpose
    #[allow(unused)]
    pub fn new_unchecked(disk: Arc<dyn BlockDevice>) -> Self {
        Self {
            boundary: HbaRange::new(Hba::new(0)..Hba::new(RawBid::MAX)),
            disk,
        }
    }
}

impl Debug for DiskView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiskView")
            .field("boundary", &self.boundary)
            .finish()
    }
}
