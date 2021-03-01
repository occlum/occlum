use std::sync::Once;

use super::rootfs::{mount_nonroot_fs_according_to, open_root_fs_according_to};
use super::*;

lazy_static! {
    static ref MOUNT_ONCE: Once = Once::new();
}

pub fn do_mount_rootfs(
    user_config: &config::Config,
    user_key: &Option<sgx_key_128bit_t>,
) -> Result<()> {
    debug!("mount rootfs");

    if MOUNT_ONCE.is_completed() {
        return_errno!(EPERM, "rootfs cannot be mounted more than once");
    }
    let new_root_inode = {
        let rootfs = open_root_fs_according_to(&user_config.mount, user_key)?;
        rootfs.root_inode()
    };
    mount_nonroot_fs_according_to(&new_root_inode, &user_config.mount, user_key)?;
    MOUNT_ONCE.call_once(|| {
        let mut root_inode = ROOT_INODE.write().unwrap();
        root_inode.fs().sync().expect("failed to sync old rootfs");
        *root_inode = new_root_inode;
        *ENTRY_POINTS.write().unwrap() = user_config.entry_points.to_owned();
    });
    Ok(())
}
