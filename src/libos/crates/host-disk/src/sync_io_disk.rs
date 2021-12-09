use block_device::{BioReq, BioSubmission, BlockDevice};
use std::fs::File;

use crate::prelude::*;
use crate::{HostDisk, OpenOptions};

pub struct SyncIoDisk {}

impl BlockDevice for SyncIoDisk {
    fn total_blocks(&self) -> usize {
        todo!()
    }

    fn submit(&self, req: Arc<BioReq>) -> BioSubmission {
        todo!()
    }
}

impl HostDisk for SyncIoDisk {
    fn from_options_and_file(options: &OpenOptions<Self>, file: File) -> Result<Self> {
        todo!()
    }
}
