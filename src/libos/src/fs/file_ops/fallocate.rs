use super::*;
use rcore_fs::vfs::{AllocFlags, FallocateMode};

pub fn do_fallocate(fd: FileDesc, flags: FallocateFlags, offset: usize, len: usize) -> Result<()> {
    debug!(
        "fallocate: fd: {}, flags: {:?}, offset: {}, len: {}",
        fd, flags, offset, len
    );
    let file_ref = current!().file(fd)?;
    file_ref.fallocate(flags, offset, len)?;
    Ok(())
}

bitflags! {
    /// Operation mode flags for fallocate
    /// Please checkout linux/include/uapi/linux/falloc.h for the details
    pub struct FallocateFlags: u32 {
        /// File size will not be changed when extend the file
        const FALLOC_FL_KEEP_SIZE = 0x01;
        /// De-allocates range
        const FALLOC_FL_PUNCH_HOLE = 0x02;
        /// Remove a range of a file without leaving a hole in the file
        const FALLOC_FL_COLLAPSE_RANGE = 0x08;
        /// Convert a range of file to zeros
        const FALLOC_FL_ZERO_RANGE = 0x10;
        /// Insert space within the file size without overwriting any existing data
        const FALLOC_FL_INSERT_RANGE = 0x20;
        /// Unshare shared blocks within the file size without overwriting any existing data
        const FALLOC_FL_UNSHARE_RANGE = 0x40;
    }
}

impl FallocateFlags {
    pub fn from_u32(raw_flags: u32) -> Result<Self> {
        let flags =
            Self::from_bits(raw_flags).ok_or_else(|| errno!(EOPNOTSUPP, "invalid flags"))?;
        if flags.contains(Self::FALLOC_FL_PUNCH_HOLE) && flags.contains(Self::FALLOC_FL_ZERO_RANGE)
        {
            return_errno!(
                EOPNOTSUPP,
                "Punch hole and zero range are mutually exclusive"
            );
        }
        if flags.contains(Self::FALLOC_FL_PUNCH_HOLE) && !flags.contains(Self::FALLOC_FL_KEEP_SIZE)
        {
            return_errno!(EOPNOTSUPP, "Punch hole must have keep size set");
        }
        if flags.contains(Self::FALLOC_FL_COLLAPSE_RANGE)
            && !(flags & !Self::FALLOC_FL_COLLAPSE_RANGE).is_empty()
        {
            return_errno!(EINVAL, "Collapse range should only be used exclusively");
        }
        if flags.contains(Self::FALLOC_FL_INSERT_RANGE)
            && !(flags & !Self::FALLOC_FL_INSERT_RANGE).is_empty()
        {
            return_errno!(EINVAL, "Insert range should only be used exclusively");
        }
        if flags.contains(Self::FALLOC_FL_UNSHARE_RANGE)
            && !(flags & !(Self::FALLOC_FL_UNSHARE_RANGE | Self::FALLOC_FL_KEEP_SIZE)).is_empty()
        {
            return_errno!(
                EINVAL,
                "Unshare range should only be used with allocate mode"
            );
        }
        Ok(flags)
    }
}

impl From<FallocateFlags> for FallocateMode {
    fn from(flags: FallocateFlags) -> FallocateMode {
        if flags.contains(FallocateFlags::FALLOC_FL_PUNCH_HOLE) {
            FallocateMode::PunchHoleKeepSize
        } else if flags.contains(FallocateFlags::FALLOC_FL_ZERO_RANGE) {
            if flags.contains(FallocateFlags::FALLOC_FL_KEEP_SIZE) {
                FallocateMode::ZeroRangeKeepSize
            } else {
                FallocateMode::ZeroRange
            }
        } else if flags.contains(FallocateFlags::FALLOC_FL_COLLAPSE_RANGE) {
            FallocateMode::CollapseRange
        } else if flags.contains(FallocateFlags::FALLOC_FL_INSERT_RANGE) {
            FallocateMode::InsertRange
        } else {
            let flags = AllocFlags::from_bits_truncate(flags.bits());
            FallocateMode::Allocate(flags)
        }
    }
}
