use super::*;

pub fn do_sync() -> Result<()> {
    info!("sync:");
    ROOT_INODE.fs().sync()?;
    Ok(())
}
