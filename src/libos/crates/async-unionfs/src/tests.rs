use crate::*;
use async_sfs::AsyncSimpleFS;
use block_device::mem_disk::MemDisk;
use std::collections::btree_set::BTreeSet;

/// Create a UnionFS for test.
/// Return root inode of (union, container, image).
///
/// container:
/// ├── file1
/// └── file2
/// image:
/// ├── file1
/// ├── file3
/// └── dir
///     ├── file4
///     └── dir2
///         └── file5
async fn create_sample() -> Result<(
    Arc<dyn AsyncFileSystem>,
    Arc<dyn AsyncInode>,
    Arc<dyn AsyncInode>,
)> {
    let container_fs = {
        let fs = create_sfs().await;
        let root = fs.root_inode().await;
        let file1 = root.create("file1", FileType::File, MODE).await?;
        let file2 = root.create("file2", FileType::File, MODE).await?;
        file1.write_at(0, b"container").await?;
        file2.write_at(0, b"container").await?;
        fs
    };
    let container_root = container_fs.root_inode().await;

    let image_fs = {
        let fs = create_sfs().await;
        let root = fs.root_inode().await;
        let file1 = root.create("file1", FileType::File, MODE).await?;
        let file3 = root.create("file3", FileType::File, MODE).await?;
        let dir = root.create("dir", FileType::Dir, MODE).await?;
        let file4 = dir.create("file4", FileType::File, MODE).await?;
        let dir2 = dir.create("dir2", FileType::Dir, MODE).await?;
        let file5 = dir2.create("file5", FileType::File, MODE).await?;
        file1.write_at(0, b"image").await?;
        file3.write_at(0, b"image").await?;
        file4.write_at(0, b"image").await?;
        file5.write_at(0, b"image").await?;
        fs
    };
    let image_root = image_fs.root_inode().await;

    let unionfs = AsyncUnionFS::new(vec![container_fs, image_fs]).await?;

    Ok((unionfs, container_root, image_root))
}

async fn create_sfs() -> Arc<AsyncSimpleFS> {
    static BLOCKS_NUM: usize = 1024 * 128;
    let mem_disk = Arc::new(MemDisk::new(BLOCKS_NUM).unwrap());
    let sfs = AsyncSimpleFS::create(mem_disk).await.unwrap();
    sfs
}

#[test]
fn read_file() -> Result<()> {
    async_rt::task::block_on(async move {
        let (fs, _, _) = create_sample().await?;
        let root = fs.root_inode().await;
        assert_eq!(
            root.lookup("file1").await?.read_as_vec().await?,
            b"container"
        );
        assert_eq!(
            root.lookup("file2").await?.read_as_vec().await?,
            b"container"
        );
        assert_eq!(root.lookup("file3").await?.read_as_vec().await?, b"image");
        assert_eq!(
            root.lookup("dir/file4").await?.read_as_vec().await?,
            b"image"
        );
        Ok(())
    })
}

#[test]
fn write_file() -> Result<()> {
    async_rt::task::block_on(async move {
        let (fs, croot, iroot) = create_sample().await?;
        let root = fs.root_inode().await;
        for path in &["file1", "file3", "dir/file4", "/dir/dir2/file5"] {
            const WRITE_DATA: &[u8] = b"I'm writing to container";
            root.lookup(path).await?.write_at(0, WRITE_DATA).await?;
            assert_eq!(croot.lookup(path).await?.read_as_vec().await?, WRITE_DATA);
            assert_eq!(iroot.lookup(path).await?.read_as_vec().await?, b"image");
            assert_eq!(
                croot.lookup(path).await?.metadata().await?.mode,
                iroot.lookup(path).await?.metadata().await?.mode
            );
        }
        assert_eq!(
            croot.lookup("dir").await?.metadata().await?.mode,
            iroot.lookup("dir").await?.metadata().await?.mode
        );
        assert_eq!(
            croot.lookup("dir/dir2").await?.metadata().await?.mode,
            iroot.lookup("dir/dir2").await?.metadata().await?.mode
        );
        Ok(())
    })
}

#[test]
fn get_direntry() -> Result<()> {
    async_rt::task::block_on(async move {
        let (fs, _croot, _iroot) = create_sample().await?;
        let root = fs.root_inode().await;
        let entries: BTreeSet<String> = root.list().await?.into_iter().collect();
        let expected: BTreeSet<String> = [".", "..", "dir", "file1", "file2", "file3"]
            .iter()
            .map(|&s| String::from(s))
            .collect();
        assert_eq!(entries, expected);
        Ok(())
    })
}

#[test]
fn unlink() -> Result<()> {
    async_rt::task::block_on(async move {
        let (fs, croot, iroot) = create_sample().await?;
        let root = fs.root_inode().await;

        root.unlink("file1").await?;
        assert!(root.lookup("file1").await.is_not_found());
        assert!(croot.lookup("file1").await.is_not_found());
        assert!(croot.lookup(".ufs.wh.file1").await.is_ok());
        assert!(iroot.lookup("file1").await.is_ok());

        root.unlink("file2").await?;
        assert!(root.lookup("file2").await.is_not_found());
        assert!(croot.lookup("file2").await.is_not_found());
        assert!(croot.lookup(".ufs.wh.file2").await.is_not_found());

        root.unlink("file3").await?;
        assert!(root.lookup("file3").await.is_not_found());
        assert!(croot.lookup(".ufs.wh.file3").await.is_ok());
        assert!(iroot.lookup("file3").await.is_ok());

        root.lookup("dir").await?.unlink("file4").await?;
        assert!(root.lookup("dir/file4").await.is_not_found());
        assert!(croot.lookup("dir/.ufs.wh.file4").await.is_ok());
        assert!(iroot.lookup("dir/file4").await.is_ok());

        root.lookup("dir")
            .await?
            .lookup("dir2")
            .await?
            .unlink("file5")
            .await?;
        assert!(root.lookup("dir/dir2/file5").await.is_not_found());
        assert!(croot.lookup("dir/dir2/.ufs.wh.file5").await.is_ok());
        assert!(iroot.lookup("dir/dir2/file5").await.is_ok());

        root.lookup("dir").await?.unlink("dir2").await?;
        assert!(root.lookup("dir/dir2").await.is_not_found());
        assert!(croot.lookup("dir/.ufs.wh.dir2").await.is_ok());
        assert!(iroot.lookup("dir/dir2").await.is_ok());

        root.unlink("dir").await?;
        assert!(root.lookup("dir").await.is_not_found());
        assert!(croot.lookup(".ufs.wh.dir").await.is_ok());
        assert!(iroot.lookup("dir").await.is_ok());

        Ok(())
    })
}

#[test]
fn unlink_then_create() -> Result<()> {
    async_rt::task::block_on(async move {
        let (fs, croot, iroot) = create_sample().await?;
        let root = fs.root_inode().await;
        root.unlink("file1").await?;
        let file1 = root.create("file1", FileType::File, MODE).await?;
        assert_eq!(file1.read_as_vec().await?, b"");
        assert!(croot.lookup(".ufs.wh.file1").await.is_not_found());

        assert!(root
            .create(".ufs.wh.file1", FileType::File, MODE)
            .await
            .is_err());
        assert!(root
            .create(".ufs.opq.file1", FileType::File, MODE)
            .await
            .is_err());
        assert!(root.create(".ufs.mac", FileType::File, MODE).await.is_err());

        root.unlink("file1").await?;
        let file1 = root.create("file1", FileType::Dir, MODE).await?;
        assert!(root.lookup("file1").await.is_ok());
        assert!(root.lookup("file1").await?.metadata().await?.type_ == FileType::Dir);
        assert!(croot.lookup("file1").await.is_ok());
        assert!(iroot.lookup("file1").await.is_ok());
        assert!(iroot.lookup("file1").await?.metadata().await?.type_ == FileType::File);
        file1.create("file6", FileType::File, MODE).await?;
        assert!(root.lookup("file1/file6").await.is_ok());
        assert!(croot.lookup("file1/file6").await.is_ok());
        assert!(iroot.lookup("file1/file6").await.is_err());

        root.lookup("dir").await?.unlink("file4").await?;
        root.lookup("dir/dir2").await?.unlink("file5").await?;
        root.lookup("dir").await?.unlink("dir2").await?;
        root.unlink("dir").await?;
        let dir = root.create("dir", FileType::Dir, MODE).await?;
        assert!(root.lookup("dir").await.is_ok());
        assert!(croot.lookup(".ufs.wh.dir").await.is_not_found());
        assert!(croot.lookup(".ufs.opq.dir").await.is_ok());
        assert!(iroot.lookup("dir").await.is_ok());
        assert!(iroot.lookup("dir").await?.list().await?.len() == 4);
        assert!(root.lookup("dir").await?.list().await?.len() == 2);

        dir.create("dir2", FileType::Dir, MODE).await?;
        assert!(root.lookup("dir/dir2").await.is_ok());
        assert!(croot.lookup("dir/.ufs.wh.dir2").await.is_not_found());
        assert!(iroot.lookup("dir/dir2").await.is_ok());
        assert!(root.lookup("dir/dir2").await?.list().await?.len() == 2);
        assert!(iroot.lookup("dir/dir2").await?.list().await?.len() == 3);

        dir.unlink("dir2").await?;
        root.unlink("dir").await?;
        assert!(root.lookup("dir").await.is_not_found());
        assert!(croot.lookup(".ufs.wh.dir").await.is_ok());
        assert!(croot.lookup(".ufs.opq.dir").await.is_not_found());

        Ok(())
    })
}

#[test]
fn link_container() -> Result<()> {
    async_rt::task::block_on(async move {
        let (fs, _, _) = create_sample().await?;
        let root = fs.root_inode().await;

        // create link
        let dir = root.lookup("dir").await?;
        let file1 = root.lookup("file1").await?;
        dir.link("file1_link", &file1).await?;

        // read from new link
        let file1_link = root.lookup("dir/file1_link").await?;
        assert_eq!(file1_link.read_as_vec().await?, b"container");

        // write then read from another link
        const WRITE_DATA: &[u8] = b"I'm writing to container";
        file1_link.write_at(0, WRITE_DATA).await?;
        assert_eq!(file1.read_as_vec().await?, WRITE_DATA);
        Ok(())
    })
}

#[test]
fn link_image() -> Result<()> {
    async_rt::task::block_on(async move {
        let (fs, _, _) = create_sample().await?;
        let root = fs.root_inode().await;

        // create link
        let dir = root.lookup("dir").await?;
        let file3 = root.lookup("file3").await?;
        dir.link("file3_link", &file3).await?;

        // read from new link
        let file3_link = root.lookup("dir/file3_link").await?;
        assert_eq!(file3_link.read_as_vec().await?, b"image");

        // write then read from another link
        const WRITE_DATA: &[u8] = b"I'm writing to container";
        file3_link.write_at(0, WRITE_DATA).await?;
        assert_eq!(file3.read_as_vec().await?, WRITE_DATA);
        Ok(())
    })
}

#[test]
fn move_container() -> Result<()> {
    async_rt::task::block_on(async move {
        let (fs, croot, _) = create_sample().await?;
        let root = fs.root_inode().await;

        let dir = root.lookup("dir").await?;
        root.move_("file1", &dir, "file1").await?;

        assert!(root.lookup("file1").await.is_not_found());
        assert!(root.lookup("dir/file1").await.is_ok());
        assert!(croot.lookup("file1").await.is_not_found());
        assert!(croot.lookup(".ufs.wh.file1").await.is_ok());
        assert!(croot.lookup("dir/file1").await.is_ok());
        Ok(())
    })
}

#[test]
fn move_image() -> Result<()> {
    async_rt::task::block_on(async move {
        let (fs, croot, iroot) = create_sample().await?;
        let root = fs.root_inode().await;

        let dir = root.lookup("dir").await?;
        dir.move_("file4", &root, "file4").await?;

        assert!(dir.lookup("file4").await.is_not_found());
        assert!(root.lookup("file4").await.is_ok());
        assert!(croot.lookup("dir/.ufs.wh.file4").await.is_ok());
        assert!(iroot.lookup("dir/file4").await.is_ok());
        Ok(())
    })
}

const MODE: u16 = 0o777;

trait IsNotFound {
    fn is_not_found(&self) -> bool;
}

impl<T> IsNotFound for Result<T> {
    fn is_not_found(&self) -> bool {
        match self {
            Err(e) if e.errno() == ENOENT => true,
            _ => false,
        }
    }
}
