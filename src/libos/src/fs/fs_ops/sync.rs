use super::*;

pub async fn do_sync() -> Result<()> {
    debug!("sync:");
    rootfs().await.sync().await?;
    Ok(())
}
