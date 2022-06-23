use crate::fs::*;
use crate::metadata::*;
use crate::prelude::*;

use async_vfs::AsyncFileSystem;
use block_device::mem_disk::MemDisk;
use std::mem::MaybeUninit;
use std::sync::Arc;

async fn _create_new_sfs() -> Arc<AsyncSimpleFS> {
    static BLOCKS_NUM: usize = 1024 * 128;
    let mem_disk = Arc::new(MemDisk::new(BLOCKS_NUM).unwrap());
    let sfs = AsyncSimpleFS::create(mem_disk).await.unwrap();
    sfs
}

#[test]
fn arc_layout() {
    // [usize, usize, T]
    //  ^ start       ^ Arc::into_raw
    let p = Arc::new([2u8; 5]);
    let ptr = Arc::into_raw(p);
    let start = unsafe { (ptr as *const usize).offset(-2) };
    let ns = unsafe { &*(start as *const [usize; 2]) };
    assert_eq!(ns, &[1usize, 1]);
}

#[test]
fn create_new_sfs() -> Result<()> {
    async_rt::task::block_on(async move {
        let sfs = _create_new_sfs().await;
        let root = sfs.root_inode().await;
        println!("fs info: {:?}", root.fs().info().await);
        assert_eq!(root.fs().info().await.magic, FS_MAGIC as usize);
        sfs.sync().await?;
        Ok(())
    })
}

#[test]
fn create_file() -> Result<()> {
    async_rt::task::block_on(async move {
        let sfs = _create_new_sfs().await;
        let root = sfs.root_inode().await;
        let file1 = root.create("file1", VfsFileType::File, 0o777).await?;
        assert_eq!(file1.metadata().await?.type_, VfsFileType::File);
        assert_eq!(file1.metadata().await?.nlinks, 1);
        sfs.sync().await?;
        Ok(())
    })
}

#[test]
fn fallocate() -> Result<()> {
    async_rt::task::block_on(async move {
        let sfs = _create_new_sfs().await;
        let root = sfs.root_inode().await;
        let file1 = root.create("file1", VfsFileType::File, 0o777).await?;
        assert_eq!(file1.metadata().await?.size, 0, "empty file size != 0");

        let mode = FallocateMode::from(async_io::fs::FallocateFlags::empty());
        let offset = 0x10;
        let len = 0x20;
        file1.fallocate(&mode, offset, len).await?;
        assert_eq!(
            file1.metadata().await?.size,
            len + offset,
            "wrong size after fallocate"
        );

        let new_offset = 0x0;
        let new_len = 0x10;
        file1.fallocate(&mode, new_offset, new_len).await?;
        assert_eq!(
            file1.metadata().await?.size,
            len + offset,
            "wrong size after fallocate"
        );

        assert!(file1.fallocate(&mode, MAX_FILE_SIZE, 0x1).await.is_err());
        sfs.sync().await?;
        Ok(())
    })
}

#[test]
fn resize() -> Result<()> {
    async_rt::task::block_on(async move {
        let sfs = _create_new_sfs().await;
        let root = sfs.root_inode().await;
        let file1 = root.create("file1", VfsFileType::File, 0o777).await?;
        assert_eq!(file1.metadata().await?.size, 0, "empty file size != 0");

        const SIZE1: usize = 0x1234;
        const SIZE2: usize = 0x1250;
        file1.resize(SIZE1).await?;
        assert_eq!(
            file1.metadata().await?.size,
            SIZE1,
            "wrong size after resize"
        );
        let mut data1: [u8; SIZE2] = unsafe { MaybeUninit::uninit().assume_init() };
        let len = file1.read_at(0, data1.as_mut()).await?;
        assert_eq!(len, SIZE1, "wrong size returned by read_at()");
        assert_eq!(
            &data1[..SIZE1],
            &[0u8; SIZE1][..],
            "expanded data should be 0"
        );
        sfs.sync().await?;
        Ok(())
    })
}

#[test]
fn resize_on_dir_should_panic() -> Result<()> {
    async_rt::task::block_on(async move {
        let sfs = _create_new_sfs().await;
        let root = sfs.root_inode().await;
        assert!(root.resize(4096).await.is_err());
        sfs.sync().await?;
        Ok(())
    })
}

#[test]
fn resize_too_large_should_rollback() -> Result<()> {
    async_rt::task::block_on(async move {
        let sfs = _create_new_sfs().await;
        let root = sfs.root_inode().await;
        let file1 = root.create("file1", VfsFileType::File, 0o777).await?;
        assert!(file1.resize(MAX_FILE_SIZE + 1).await.is_err());
        assert!(file1.metadata().await?.size == 0);
        assert!(file1.resize(MAX_FILE_SIZE).await.is_err());
        assert!(file1.metadata().await?.size == 0);
        sfs.sync().await?;
        Ok(())
    })
}

#[test]
fn create_then_lookup() -> Result<()> {
    async_rt::task::block_on(async move {
        let sfs = _create_new_sfs().await;
        let root = sfs.root_inode().await;

        assert!(
            Arc::ptr_eq(&root.lookup(".").await?, &root),
            "failed to find ."
        );
        assert!(
            Arc::ptr_eq(&root.lookup("..").await?, &root),
            "failed to find .."
        );

        let file1 = root
            .create("file1", VfsFileType::File, 0o777)
            .await
            .expect("failed to create file1");
        assert!(
            Arc::ptr_eq(&root.lookup("file1").await?, &file1),
            "failed to find file1"
        );
        assert!(
            root.lookup("file2").await.is_err(),
            "found non-existent file"
        );

        let dir1 = root
            .create("dir1", VfsFileType::Dir, 0o777)
            .await
            .expect("failed to create dir1");
        let file2 = dir1
            .create("file2", VfsFileType::File, 0o777)
            .await
            .expect("failed to create /dir1/file2");
        assert!(
            Arc::ptr_eq(&root.lookup("dir1/file2").await?, &file2),
            "failed to find dir1/file2"
        );
        assert!(
            Arc::ptr_eq(&root.lookup("/").await?.lookup("dir1/file2").await?, &file2),
            "failed to find dir1/file2"
        );
        assert!(
            Arc::ptr_eq(&dir1.lookup("..").await?, &root),
            "failed to find .. from dir1"
        );

        assert!(
            Arc::ptr_eq(&dir1.lookup("../dir1/file2").await?, &file2),
            "failed to find dir1/file2 by relative"
        );
        assert!(
            Arc::ptr_eq(&dir1.lookup("/dir1/file2").await?, &file2),
            "failed to find dir1/file2 by absolute"
        );
        assert!(
            Arc::ptr_eq(&dir1.lookup("/dir1/../dir1/file2").await?, &file2),
            "failed to find dir1/file2 by absolute"
        );
        assert!(
            Arc::ptr_eq(&dir1.lookup("../../..//dir1/../dir1/file2").await?, &file2),
            "failed to find dir1/file2 by more than one .."
        );
        assert!(
            Arc::ptr_eq(&dir1.lookup("..//dir1/file2").await?, &file2),
            "failed to find dir1/file2 by weird relative"
        );

        assert!(
            root.lookup("./dir1/../file2").await.is_err(),
            "found non-existent file"
        );
        assert!(
            root.lookup("./dir1/../file3").await.is_err(),
            "found non-existent file"
        );
        assert!(
            root.lookup("/dir1/../dir1/../file3").await.is_err(),
            "found non-existent file"
        );
        assert!(
            root.lookup("/dir1/../../../dir1/../file3").await.is_err(),
            "found non-existent file"
        );
        assert!(
            root.lookup("/")
                .await
                .unwrap()
                .lookup("dir1/../file2")
                .await
                .is_err(),
            "found non-existent file"
        );
        sfs.sync().await?;
        Ok(())
    })
}

#[test]
fn test_symlinks() -> Result<()> {
    async_rt::task::block_on(async move {
        let sfs = _create_new_sfs().await;
        let root = sfs.root_inode().await;

        let file1 = root
            .create("file1", VfsFileType::File, 0o777)
            .await
            .expect("failed to create file1");
        assert!(
            Arc::ptr_eq(&root.lookup("file1").await?, &file1),
            "failed to find file1"
        );

        let link1 = root
            .create("link1", VfsFileType::SymLink, 0o777)
            .await
            .expect("failed to create link1");
        let data = "file1";
        link1.write_link(data).await?;

        let link2 = root
            .create("link2", VfsFileType::SymLink, 0o777)
            .await
            .expect("failed to create link2");
        let data = "link1";
        link2.write_link(data).await?;
        assert!(
            Arc::ptr_eq(&root.lookup("link1").await?, &link1),
            "failed to find link1 by relative"
        );
        assert!(
            Arc::ptr_eq(&root.lookup_follow("link1", Some(1)).await?, &file1),
            "failed to find file1 by link1"
        );
        assert!(
            Arc::ptr_eq(&root.lookup_follow("link2", None).await?, &link2),
            "failed to find link2 by link2"
        );
        assert!(
            Arc::ptr_eq(&root.lookup_follow("link2", Some(2)).await?, &file1),
            "failed to find file1 by link2"
        );

        let link3 = root
            .create("link3", VfsFileType::SymLink, 0o777)
            .await
            .expect("failed to create link3");
        let data = "/link2";
        link3.write_link(data).await?;
        assert!(
            Arc::ptr_eq(&root.lookup_follow("link3", None).await?, &link3),
            "failed to find link3 by link3"
        );
        assert!(
            Arc::ptr_eq(&root.lookup_follow("link3", Some(3)).await?, &file1),
            "failed to find file1 by link2"
        );

        let dir1 = root
            .create("dir1", VfsFileType::Dir, 0o777)
            .await
            .expect("failed to create dir1");
        let file2 = dir1
            .create("file2", VfsFileType::File, 0o777)
            .await
            .expect("failed to create /dir1/file2");
        let link_dir = root
            .create("link_dir", VfsFileType::SymLink, 0o777)
            .await
            .expect("failed to create link2");
        let data = "dir1";
        link_dir.write_link(data).await?;
        assert!(
            Arc::ptr_eq(
                &root.lookup_follow("link_dir/file2", Some(1)).await?,
                &file2
            ),
            "failed to find file2"
        );

        sfs.sync().await?;
        Ok(())
    })
}

#[test]
fn test_double_indirect_blocks() -> Result<()> {
    async_rt::task::block_on(async move {
        let sfs = _create_new_sfs().await;
        let root = sfs.root_inode().await;

        let file1 = root
            .create("file1", VfsFileType::File, 0o777)
            .await
            .expect("failed to create file1");
        assert!(
            Arc::ptr_eq(&root.lookup("file1").await?, &file1),
            "failed to find file1"
        );

        // resize to direct maximum size
        file1.resize(MAX_NBLOCK_DIRECT * BLOCK_SIZE).await.unwrap();
        // force usage of indirect block
        file1
            .resize((MAX_NBLOCK_DIRECT + 1) * BLOCK_SIZE)
            .await
            .unwrap();
        file1
            .resize(MAX_NBLOCK_INDIRECT * BLOCK_SIZE)
            .await
            .unwrap();
        // force usage of double indirect block
        file1
            .resize((MAX_NBLOCK_INDIRECT + 1) * BLOCK_SIZE)
            .await
            .unwrap();
        file1
            .resize((MAX_NBLOCK_INDIRECT + 2) * BLOCK_SIZE)
            .await
            .unwrap();

        // resize up and down
        file1.resize(0).await.unwrap();
        file1
            .resize((MAX_NBLOCK_INDIRECT + 2) * BLOCK_SIZE)
            .await
            .unwrap();
        file1.resize(MAX_NBLOCK_DIRECT * BLOCK_SIZE).await.unwrap();
        file1
            .resize((MAX_NBLOCK_DIRECT + 1) * BLOCK_SIZE)
            .await
            .unwrap();
        file1.resize(MAX_NBLOCK_DIRECT * BLOCK_SIZE).await.unwrap();
        file1.resize(0).await.unwrap();
        file1
            .resize((MAX_NBLOCK_INDIRECT + 1) * BLOCK_SIZE)
            .await
            .unwrap();
        file1.resize(MAX_NBLOCK_DIRECT * BLOCK_SIZE).await.unwrap();
        file1
            .resize((MAX_NBLOCK_INDIRECT + 2) * BLOCK_SIZE)
            .await
            .unwrap();
        file1
            .resize((MAX_NBLOCK_INDIRECT + 1) * BLOCK_SIZE)
            .await
            .unwrap();
        file1.resize(0).await.unwrap();

        sfs.sync().await?;
        Ok(())
    })
}

#[test]
fn hard_link() -> Result<()> {
    async_rt::task::block_on(async move {
        let sfs = _create_new_sfs().await;
        let root = sfs.root_inode().await;
        let file1 = root.create("file1", VfsFileType::File, 0o777).await?;
        root.link("file2", &file1).await?;
        let file2 = root.lookup("file2").await?;
        file1.resize(100).await?;
        assert_eq!(file2.metadata().await?.size, 100);
        sfs.sync().await?;
        Ok(())
    })
}

#[test]
fn nlinks() -> Result<()> {
    async_rt::task::block_on(async move {
        let sfs = _create_new_sfs().await;
        let root = sfs.root_inode().await;
        // -root
        assert_eq!(root.metadata().await?.nlinks, 2);

        let file1 = root.create("file1", VfsFileType::File, 0o777).await?;
        // -root
        //   `-file1 <f1>
        assert_eq!(file1.metadata().await?.nlinks, 1);
        assert_eq!(root.metadata().await?.nlinks, 2);

        let dir1 = root.create("dir1", VfsFileType::Dir, 0o777).await?;
        // -root
        //   +-dir1
        //   `-file1 <f1>
        assert_eq!(dir1.metadata().await?.nlinks, 2);
        assert_eq!(root.metadata().await?.nlinks, 3);

        root.move_("dir1", &root, "dir_1").await?;
        // -root
        //   +-dir_1
        //   `-file1 <f1>
        assert_eq!(dir1.metadata().await?.nlinks, 2);
        assert_eq!(root.metadata().await?.nlinks, 3);

        dir1.link("file1_", &file1).await?;
        // -root
        //   +-dir_1
        //   |  `-file1_ <f1>
        //   `-file1 <f1>
        assert_eq!(dir1.metadata().await?.nlinks, 2);
        assert_eq!(root.metadata().await?.nlinks, 3);
        assert_eq!(file1.metadata().await?.nlinks, 2);

        let dir2 = root.create("dir2", VfsFileType::Dir, 0o777).await?;
        // -root
        //   +-dir_1
        //   |  `-file1_ <f1>
        //   +-dir2
        //   `-file1 <f1>
        assert_eq!(dir1.metadata().await?.nlinks, 2);
        assert_eq!(dir2.metadata().await?.nlinks, 2);
        assert_eq!(root.metadata().await?.nlinks, 4);
        assert_eq!(file1.metadata().await?.nlinks, 2);

        root.move_("file1", &root, "file_1").await?;
        // -root
        //   +-dir_1
        //   |  `-file1_ <f1>
        //   +-dir2
        //   `-file_1 <f1>
        assert_eq!(dir1.metadata().await?.nlinks, 2);
        assert_eq!(dir2.metadata().await?.nlinks, 2);
        assert_eq!(root.metadata().await?.nlinks, 4);
        assert_eq!(file1.metadata().await?.nlinks, 2);

        root.move_("file_1", &dir2, "file__1").await?;
        // -root
        //   +-dir_1
        //   |  `-file1_ <f1>
        //   `-dir2
        //      `-file__1 <f1>
        assert_eq!(dir1.metadata().await?.nlinks, 2);
        assert_eq!(dir2.metadata().await?.nlinks, 2);
        assert_eq!(root.metadata().await?.nlinks, 4);
        assert_eq!(file1.metadata().await?.nlinks, 2);

        root.move_("dir_1", &dir2, "dir__1").await?;
        // -root
        //   `-dir2
        //      +-dir__1
        //      |  `-file1_ <f1>
        //      `-file__1 <f1>
        assert_eq!(dir1.metadata().await?.nlinks, 2);
        assert_eq!(dir2.metadata().await?.nlinks, 3);
        assert_eq!(root.metadata().await?.nlinks, 3);
        assert_eq!(file1.metadata().await?.nlinks, 2);

        dir2.unlink("file__1").await?;
        // -root
        //   `-dir2
        //      `-dir__1
        //         `-file1_ <f1>
        assert_eq!(file1.metadata().await?.nlinks, 1);
        assert_eq!(dir1.metadata().await?.nlinks, 2);
        assert_eq!(dir2.metadata().await?.nlinks, 3);
        assert_eq!(root.metadata().await?.nlinks, 3);

        dir1.unlink("file1_").await?;
        // -root
        //   `-dir2
        //      `-dir__1
        assert_eq!(file1.metadata().await?.nlinks, 0);
        assert_eq!(dir1.metadata().await?.nlinks, 2);
        assert_eq!(dir2.metadata().await?.nlinks, 3);
        assert_eq!(root.metadata().await?.nlinks, 3);

        dir2.unlink("dir__1").await?;
        // -root
        //   `-dir2
        assert_eq!(file1.metadata().await?.nlinks, 0);
        assert_eq!(dir1.metadata().await?.nlinks, 0);
        assert_eq!(root.metadata().await?.nlinks, 3);
        assert_eq!(dir2.metadata().await?.nlinks, 2);

        root.unlink("dir2").await?;
        // -root
        assert_eq!(file1.metadata().await?.nlinks, 0);
        assert_eq!(dir1.metadata().await?.nlinks, 0);
        assert_eq!(root.metadata().await?.nlinks, 2);
        assert_eq!(dir2.metadata().await?.nlinks, 0);

        sfs.sync().await?;
        Ok(())
    })
}

#[test]
fn ext() -> Result<()> {
    async_rt::task::block_on(async move {
        let sfs = _create_new_sfs().await;
        let root = sfs.root_inode().await;
        let file1 = root.create("file1", VfsFileType::File, 0o777).await?;

        #[derive(Clone)]
        struct MyStruct(usize);
        impl async_io::fs::AnyExt for MyStruct {}

        let ext = file1.ext().unwrap();
        assert_eq!(ext.get::<MyStruct>().is_none(), true);
        let val = MyStruct(0xff);
        ext.put::<MyStruct>(Arc::new(val.clone()));
        assert_eq!(ext.get::<MyStruct>().is_some(), true);
        assert_eq!(ext.get::<MyStruct>().unwrap().0, val.0);
        sfs.sync().await?;
        Ok(())
    })
}

// benchmark test
// use sgx_disk::{HostDisk, SyncIoDisk};

// const MB: usize = 1024 * 1024;
// const GB: usize = MB * 1024;

// async fn _create_new_sfs_disk() -> Arc<AsyncSimpleFS> {
//     let num_blocks = 2 * GB / BLOCK_SIZE;
//     let image_path = std::path::PathBuf::from("./image");
//     if image_path.exists() {
//         std::fs::remove_file(&image_path).unwrap();
//     }
//     let sync_disk = SyncIoDisk::create(&image_path, num_blocks).unwrap();
//     let sfs = AsyncSimpleFS::create(Arc::new(sync_disk)).await.unwrap();
//     sfs
// }

// // run 'cargo test bench --release -- --nocapture' to get the result
// #[test]
// fn seq_write_bench() -> Result<()> {
//     async_rt::task::block_on(async move {
//         let sfs = _create_new_sfs_disk().await;
//         let root = sfs.root_inode().await;
//         let file1 = root.create("file1", VfsFileType::File, 0o777).await?;
//         static BUFFER: [u8; BLOCK_SIZE] = [0x1; BLOCK_SIZE];
//         let now = std::time::SystemTime::now();
//         for i in 0..1 * GB / BLOCK_SIZE {
//             file1.write_at(i * BLOCK_SIZE, &BUFFER).await?;
//         }
//         file1.sync_all().await?;
//         let secs = now.elapsed().unwrap().as_secs_f64();
//         println!("{:?} seconds elapsed", secs);
//         println!("seq-write throughput: {:?} MB/s", (GB / MB) as f64 / secs);
//         sfs.sync().await?;
//         Ok(())
//     })
// }
