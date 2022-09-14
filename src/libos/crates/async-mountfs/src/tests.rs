use crate::*;
use async_sfs::AsyncSimpleFS;
use block_device::mem_disk::MemDisk;

async fn _create_new_sfs() -> Arc<AsyncSimpleFS> {
    static BLOCKS_NUM: usize = 1024 * 128;
    let mem_disk = Arc::new(MemDisk::new(BLOCKS_NUM).unwrap());
    let sfs = AsyncSimpleFS::create(mem_disk).await.unwrap();
    sfs
}

#[test]
fn mount_lookup_umount() -> Result<()> {
    async_rt::task::block_on(async move {
        let rootfs = AsyncMountFS::new(_create_new_sfs().await);
        let root = rootfs.root_inode().await;
        let dir = root.create("dir", FileType::Dir, 0o777).await.unwrap();
        let mnt = dir.create("mnt", FileType::Dir, 0o777).await.unwrap();
        let new_sfs = {
            let sfs = _create_new_sfs().await;
            let root = sfs.root_inode().await;
            let dir = root.create("dir", FileType::Dir, 0o777).await.unwrap();
            dir.create("file", FileType::File, 0o666).await.unwrap();
            sfs
        };
        // Mount fs
        mnt.mount(new_sfs).await.unwrap();

        // Lookup
        // Going down to trespass fs border
        let mnt_dir = root.lookup("dir/mnt/dir").await.unwrap();
        // Not trespass filesystem border
        assert!(mnt_dir.lookup("file").await.is_ok());
        // Going up to trespass fs border
        let new_dir = root.lookup("dir/mnt/..").await.unwrap();
        assert!(new_dir.metadata().await.unwrap().inode == dir.metadata().await.unwrap().inode);

        // Umount fs
        let mnt = root.lookup("dir/mnt").await.unwrap();
        mnt.umount().await.unwrap();
        assert!(root.lookup("dir/mnt/dir").await.is_err());

        Ok(())
    })
}
