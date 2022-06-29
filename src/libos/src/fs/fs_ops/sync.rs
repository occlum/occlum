use super::*;

pub async fn do_sync() -> Result<()> {
    debug!("sync:");
    ROOT_FS.read().unwrap().sync()?;
    if async_sfs_initilized() {
        async_sfs().await.sync().await?;
    }
    Ok(())
}
