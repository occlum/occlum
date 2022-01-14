use super::*;

pub fn do_sync() -> Result<()> {
    debug!("sync:");
    ROOT_FS.read().unwrap().sync()?;
    Ok(())
}
