bitflags::bitflags! {
    pub struct LinkFlags: i32 {
        const AT_EMPTY_PATH = 0x1000;
        const AT_SYMLINK_FOLLOW = 0x400;
    }
}

bitflags::bitflags! {
    pub struct UnlinkFlags: i32 {
        const AT_REMOVEDIR = 0x200;
    }
}
