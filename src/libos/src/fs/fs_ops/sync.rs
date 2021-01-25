use super::*;

pub fn do_sync() -> Result<()> {
    debug!("sync:");
    ROOT_INODE.read().unwrap().fs().sync()?;
    Ok(())
}
