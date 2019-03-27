use rcore_fs::dev::TimeProvider;
use rcore_fs::vfs::Timespec;
use rcore_fs_sefs::dev::*;
use std::boxed::Box;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sgxfs::{OpenOptions, remove, SgxFile};
use std::sync::SgxMutex as Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SgxStorage {
    path: PathBuf,
}

impl SgxStorage {
    pub fn new(path: impl AsRef<Path>) -> Self {
        //        assert!(path.as_ref().is_dir());
        SgxStorage {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl Storage for SgxStorage {
    fn open(&self, file_id: usize) -> DevResult<Box<File>> {
        let mut path = self.path.to_path_buf();
        path.push(format!("{}", file_id));
        // TODO: key
        let key = [0u8; 16];
        let file = OpenOptions::new()
            .read(true)
            .update(true)
            .open_ex(path, &key)
            .expect("failed to open SgxFile");
        Ok(Box::new(LockedFile(Mutex::new(file))))
    }

    fn create(&self, file_id: usize) -> DevResult<Box<File>> {
        let mut path = self.path.to_path_buf();
        path.push(format!("{}", file_id));
        // TODO: key
        let key = [0u8; 16];
        let file = OpenOptions::new()
            .write(true)
            .update(true)
            .open_ex(path, &key)
            .expect("failed to create SgxFile");
        Ok(Box::new(LockedFile(Mutex::new(file))))
    }

    fn remove(&self, file_id: usize) -> DevResult<()> {
        let mut path = self.path.to_path_buf();
        path.push(format!("{}", file_id));
        remove(path).expect("failed to remove SgxFile");
        Ok(())
    }
}

pub struct LockedFile(Mutex<SgxFile>);

// `sgx_tstd::sgxfs::SgxFile` not impl Send ...
unsafe impl Send for LockedFile {}
unsafe impl Sync for LockedFile {}

impl File for LockedFile {
    fn read_at(&self, buf: &mut [u8], offset: usize) -> DevResult<usize> {
        let mut file = self.0.lock().unwrap();
        let offset = offset as u64;
        file.seek(SeekFrom::Start(offset))
            .expect("failed to seek SgxFile");
        let len = file.read(buf).expect("failed to read SgxFile");
        Ok(len)
    }

    fn write_at(&self, buf: &[u8], offset: usize) -> DevResult<usize> {
        let mut file = self.0.lock().unwrap();
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
