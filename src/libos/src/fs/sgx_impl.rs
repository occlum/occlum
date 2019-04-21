use rcore_fs::dev::TimeProvider;
use rcore_fs::vfs::Timespec;
use rcore_fs_sefs::dev::*;
use std::boxed::Box;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sgxfs::{remove, OpenOptions, SgxFile};
use std::sync::{SgxMutex as Mutex, Arc};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::BTreeMap;

pub struct SgxStorage {
    path: PathBuf,
    file_cache: Mutex<BTreeMap<usize, LockedFile>>,
}

impl SgxStorage {
    pub fn new(path: impl AsRef<Path>) -> Self {
        //        assert!(path.as_ref().is_dir());
        SgxStorage {
            path: path.as_ref().to_path_buf(),
            file_cache: Mutex::new(BTreeMap::new())
        }
    }
    /// Get file by `file_id`.
    /// It lookups cache first, if miss, then call `open_fn` to open one,
    /// and add it to cache before return.
    fn get(&self, file_id: usize, open_fn: impl FnOnce(&Self) -> LockedFile) -> LockedFile {
        // query cache
        let mut caches = self.file_cache.lock().unwrap();
        if let Some(locked_file) = caches.get(&file_id) {
            // hit, return
            return locked_file.clone();
        }
        // miss, open one
        let locked_file = open_fn(self);
        // add to cache
        caches.insert(file_id, locked_file.clone());
        locked_file
    }
}

impl Storage for SgxStorage {
    fn open(&self, file_id: usize) -> DevResult<Box<File>> {
        let locked_file = self.get(file_id, |this| {
            let mut path = this.path.to_path_buf();
            path.push(format!("{}", file_id));
            // TODO: key
            let key = [0u8; 16];
            let file = OpenOptions::new()
                .read(true)
                .update(true)
                .open_ex(path, &key)
                .expect("failed to open SgxFile");
            LockedFile(Arc::new(Mutex::new(file)))
        });
        Ok(Box::new(locked_file))
    }

    fn create(&self, file_id: usize) -> DevResult<Box<File>> {
        let locked_file = self.get(file_id, |this| {
            let mut path = this.path.to_path_buf();
            path.push(format!("{}", file_id));
            // TODO: key
            let key = [0u8; 16];
            let file = OpenOptions::new()
                .write(true)
                .update(true)
                .open_ex(path, &key)
                .expect("failed to create SgxFile");
            LockedFile(Arc::new(Mutex::new(file)))
        });
        Ok(Box::new(locked_file))
    }

    fn remove(&self, file_id: usize) -> DevResult<()> {
        let mut path = self.path.to_path_buf();
        path.push(format!("{}", file_id));
        remove(path).expect("failed to remove SgxFile");
        // remove from cache
        let mut caches = self.file_cache.lock().unwrap();
        caches.remove(&file_id);
        Ok(())
    }
}

#[derive(Clone)]
pub struct LockedFile(Arc<Mutex<SgxFile>>);

// `sgx_tstd::sgxfs::SgxFile` not impl Send ...
unsafe impl Send for LockedFile {}
unsafe impl Sync for LockedFile {}

impl File for LockedFile {
    fn read_at(&self, buf: &mut [u8], offset: usize) -> DevResult<usize> {
        if buf.len() == 0 {
            return Ok(0);
        }
        let mut file = self.0.lock().unwrap();
        let offset = offset as u64;
        file.seek(SeekFrom::Start(offset))
            .expect("failed to seek SgxFile");
        let len = file.read(buf).expect("failed to read SgxFile");
        Ok(len)
    }

    fn write_at(&self, buf: &[u8], offset: usize) -> DevResult<usize> {
        if buf.len() == 0 {
            return Ok(0);
        }
        let mut file = self.0.lock().unwrap();

        // SgxFile do not support seek a position after the end.
        // So check the size and padding zeros if necessary.
        let file_size = file.seek(SeekFrom::End(0)).expect("failed to tell SgxFile") as usize;
        if file_size < offset {
            static ZEROS: [u8; 0x1000] = [0; 0x1000];
            let mut rest_len = offset - file_size;
            while rest_len != 0 {
                let l = rest_len.min(0x1000);
                let len = file.write(&ZEROS[..l]).expect("failed to write SgxFile");
                rest_len -= len;
            }
        }

        let offset = offset as u64;
        file.seek(SeekFrom::Start(offset))
            .expect("failed to seek SgxFile");
        let len = file.write(buf).expect("failed to write SgxFile");
        Ok(len)
    }

    fn set_len(&self, len: usize) -> DevResult<()> {
        // NOTE: do nothing ??
        Ok(())
    }

    fn flush(&self) -> DevResult<()> {
        let mut file = self.0.lock().unwrap();
        file.flush().expect("failed to flush SgxFile");
        Ok(())
    }
}
