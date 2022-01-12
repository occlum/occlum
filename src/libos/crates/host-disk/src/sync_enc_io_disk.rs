use block_device::{BioReq, BioSubmission, BioType, BlockDevice};
use fs::File;
use std::io::prelude::*;
use std::io::{IoSlice, IoSliceMut, SeekFrom};
use std::path::{Path, PathBuf};

use crate::prelude::*;
use crate::{HostDisk, OpenOptions};

use sgx_tcrypto::{rsgx_rijndael128GCM_decrypt, rsgx_rijndael128GCM_encrypt};

/// A type of host disk that implements a block device interface by performing
/// normal synchronous I/O to the underlying host file.
///
/// `SyncIoDisk` implements the interface of `BlockDevice`. Although the
/// interface is asynchronous, the implementation uses the normal synchronous
/// `read` and `write` system calls to perform I/O. Thus, the performance of
/// `SyncIoDisk` is not good. This is especially true for SGX where issuing
/// system calls from the enclave triggers enclave switching, which is costly.
///
/// It is recommended to use `IoUringDisk` for an optimal performance.
#[derive(Debug)]
pub struct SyncEncIoDisk {
    file: Mutex<File>,
    path: PathBuf,
    total_blocks: usize,
    can_read: bool,
    can_write: bool,
}

impl SyncEncIoDisk {
    fn do_read(&self, req: &Arc<BioReq>) -> Result<()> {
        if !self.can_read {
            return Err(errno!(EACCES, "read is not allowed"));
        }

        let (offset, _) = self.get_range_in_bytes(&req)?;

        let mut file = self.file.lock().unwrap();
        file.seek(SeekFrom::Start(offset as u64))?;
        let read_len = req.access_mut_bufs_with(|bufs| {
            let mut slices: Vec<IoSliceMut<'_>> = bufs
                .iter_mut()
                .map(|buf| IoSliceMut::new(buf.as_slice_mut()))
                .collect();

            file.read_vectored(&mut slices)
        })?;

        drop(file);

        debug_assert!(read_len / BLOCK_SIZE == req.num_blocks());
        Ok(())
    }

    fn do_write(&self, req: &Arc<BioReq>) -> Result<()> {
        if !self.can_write {
            return Err(errno!(EACCES, "write is not allowed"));
        }
        let (offset, _) = self.get_range_in_bytes(&req)?;

        let mut file = self.file.lock().unwrap();

        file.seek(SeekFrom::Start(offset as u64))?;
        const IOV_MAX_IN_LINUX: usize = 1024;

        let write_len = req.access_bufs_with(|bufs| {
            let writev_times = bufs.len() / IOV_MAX_IN_LINUX;
            let rem_len = bufs.len() % IOV_MAX_IN_LINUX;
            let mut total_write_len = 0;
            let mut idx = 0;

            while idx < writev_times {
                let (ciphers, gmacs) = {
                    let mut ciphers = Vec::with_capacity(bufs.len());
                    let mut gmacs = Vec::with_capacity(bufs.len());
                    for buf in bufs[idx * IOV_MAX_IN_LINUX..(idx + 1) * IOV_MAX_IN_LINUX].iter() {
                        let (cipher, gmac) = aes_gcm_encrypt(buf.as_slice());
                        ciphers.push(cipher);
                        gmacs.push(gmac);
                    }
                    (ciphers, gmacs)
                };

                let slices: Vec<IoSlice<'_>> = ciphers
                    .iter()
                    .map(|buf| IoSlice::new(buf.as_slice()))
                    .collect();
                total_write_len += file.write_vectored(&slices).unwrap();
                idx += 1;
            }

            if rem_len > 0 {
                let slices: Vec<IoSlice<'_>> = bufs
                    [writev_times * IOV_MAX_IN_LINUX..writev_times * IOV_MAX_IN_LINUX + rem_len]
                    .iter()
                    .map(|buf| IoSlice::new(buf.as_slice()))
                    .collect();
                total_write_len += file.write_vectored(&slices).unwrap();
            }
            total_write_len
        });
        drop(file);

        debug_assert!(write_len / BLOCK_SIZE == req.num_blocks());
        Ok(())
    }

    fn do_flush(&self) -> Result<()> {
        if !self.can_write {
            return Err(errno!(EACCES, "flush is not allowed"));
        }

        let mut file = self.file.lock().unwrap();
        file.flush()?;
        file.sync_all()?;
        drop(file);

        Ok(())
    }

    fn get_range_in_bytes(&self, req: &Arc<BioReq>) -> Result<(usize, usize)> {
        let begin_block = req.addr();
        let end_block = begin_block + req.num_blocks();
        if end_block > self.total_blocks {
            return Err(errno!(EINVAL, "invalid block range"));
        }
        let begin_offset = begin_block * BLOCK_SIZE;
        let end_offset = end_block * BLOCK_SIZE;
        Ok((begin_offset, end_offset))
    }
}

fn aes_gcm_encrypt(plain: &[u8]) -> (Vec<u8>, [u8; 16]) {
    let mut gmac = [0; 16];
    let aes_key: [u8; 16] = [1; 16];
    let nonce: [u8; 12] = [2; 12];
    let aad: [u8; 0] = [0; 0];
    let ciphertxt = {
        let mut ciphertxt = vec![0u8; plain.len()];
        rsgx_rijndael128GCM_encrypt(&aes_key, plain, &nonce, &aad, &mut ciphertxt, &mut gmac)
            .unwrap();
        ciphertxt
    };

    (ciphertxt, gmac)
}

fn aes_gcm_decrypt(gmac: &[u8; 16], cipher: &[u8], plain: &mut [u8]) {
    let aes_key: [u8; 16] = [1; 16];
    let nonce: [u8; 12] = [2; 12];
    let aad: [u8; 0] = [0; 0];
    rsgx_rijndael128GCM_decrypt(&aes_key, cipher, &nonce, &aad, gmac, plain).unwrap();
}

impl BlockDevice for SyncEncIoDisk {
    fn total_blocks(&self) -> usize {
        self.total_blocks
    }

    fn submit(&self, req: Arc<BioReq>) -> BioSubmission {
        // Update the status of req to submittted
        let submission = BioSubmission::new(req);

        let req = submission.req();
        let type_ = req.type_();
        let res = match type_ {
            BioType::Read => self.do_read(req),
            BioType::Write => self.do_write(req),
            BioType::Flush => self.do_flush(),
        };

        // Update the status of req to completed and set the response
        let resp = res.map_err(|e| e.errno());
        unsafe {
            req.complete(resp);
        }

        submission
    }
}

impl HostDisk for SyncEncIoDisk {
    fn from_options_and_file(options: &OpenOptions<Self>, file: File, path: &Path) -> Result<Self> {
        let total_blocks = options.total_blocks.unwrap_or_else(|| {
            let file_len = file.metadata().unwrap().len() as usize;
            assert!(file_len >= BLOCK_SIZE);
            file_len / BLOCK_SIZE
        });
        let can_read = options.read;
        let can_write = options.write;
        let path = path.to_owned();
        let new_self = Self {
            file: Mutex::new(file),
            path,
            total_blocks,
            can_read,
            can_write,
        };
        Ok(new_self)
    }

    fn path(&self) -> &Path {
        self.path.as_path()
    }
}

impl Drop for SyncEncIoDisk {
    fn drop(&mut self) {
        // Ensure all data are peristed before the disk is dropped
        let _ = self.do_flush();
    }
}
