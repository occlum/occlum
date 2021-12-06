use crate::prelude::*;

/// A block device.
pub trait BlockDevice {
    /// Return the total number of blocks in the device.
    fn total_blocks(&self) -> usize;

    /// Submit an I/O request to the device, returning the I/O submission that
    /// corresponds to the I/O request.
    ///
    /// The status of the request can be queried via the submission object.
    /// Async rust code can also await on the submission object for the
    /// completion of the I/O request.
    fn submit(&self, req: Arc<BioReq>) -> BioSubmission;
}

impl dyn BlockDevice {
    /// Return the total number of bytes in the device.
    pub fn total_bytes(&self) -> usize {
        self.total_blocks() * crate::BLOCK_SIZE
    }
}
