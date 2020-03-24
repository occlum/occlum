use super::*;

pub fn do_sync() -> Result<()> {
    debug!("sync:");
    ROOT_INODE.fs().sync()?;
    Ok(())
}
