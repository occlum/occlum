use block_device::{BioReq, BioSubmission, BlockDevice};
use std::fs::File;

use crate::prelude::*;
use crate::{HostDisk, OpenOptions};

pub struct IoUringDisk {}

impl BlockDevice for IoUringDisk {
    fn total_blocks(&self) -> usize {
        todo!()
    }

    fn submit(&self, req: Arc<BioReq>) -> BioSubmission {
        todo!()
    }
}

impl HostDisk for IoUringDisk {
    fn new(options: &OpenOptions<Self>, file: File) -> Result<Self> {
        todo!()
    }
}
