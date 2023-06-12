//! Checkpoint region.
use crate::prelude::*;
use crate::superblock::{CheckpointRegion, SuperBlock};

use std::fmt::{self, Debug};

mod bitc;
mod dst;
mod key_table;
mod rit;
mod svt;

pub(crate) use self::bitc::BITC;
pub(crate) use self::dst::DST;
pub(crate) use self::key_table::KeyTable;
pub(crate) use self::rit::RIT;
pub(crate) use self::svt::SVT;

/// Checkpoint.
/// Manage several auxiliary data structures.
pub struct Checkpoint {
    bitc: RwLock<BITC>,
    data_svt: RwLock<SVT>,
    index_svt: RwLock<SVT>,
    dst: RwLock<DST>,
    rit: AsyncRwLock<RIT>,
    key_table: KeyTable,
    disk: DiskView,
}
// TODO: Introduce shadow paging for recovery
// TODO: Support on-demand loading for every structure

impl Checkpoint {
    pub fn new(superblock: &SuperBlock, disk: DiskView, root_key: &Key) -> Self {
        Self {
            bitc: RwLock::new(BITC::new()),
            data_svt: RwLock::new(SVT::new(
                superblock.data_region_addr,
                superblock.num_data_segments,
                SEGMENT_SIZE,
            )),
            index_svt: RwLock::new(SVT::new(
                superblock.index_region_addr,
                superblock.num_index_segments,
                INDEX_SEGMENT_SIZE,
            )),
            dst: RwLock::new(DST::new(
                superblock.data_region_addr,
                superblock.num_data_segments,
            )),
            rit: AsyncRwLock::new(RIT::new(
                superblock.checkpoint_region.rit_addr,
                superblock.data_region_addr,
                disk.clone(),
                root_key,
            )),
            key_table: KeyTable::new(superblock.data_region_addr),
            disk,
        }
    }

    pub fn bitc(&self) -> &RwLock<BITC> {
        &self.bitc
    }

    pub fn data_svt(&self) -> &RwLock<SVT> {
        &self.data_svt
    }

    pub fn index_svt(&self) -> &RwLock<SVT> {
        &self.index_svt
    }

    pub fn dst(&self) -> &RwLock<DST> {
        &self.dst
    }

    pub fn rit(&self) -> &AsyncRwLock<RIT> {
        &self.rit
    }

    pub fn key_table(&self) -> &KeyTable {
        &self.key_table
    }
}

impl Checkpoint {
    pub async fn persist(&self, superblock: &SuperBlock, root_key: &Key) -> Result<()> {
        let region = &superblock.checkpoint_region;
        if self.still_initialized() {
            return self.commit_pflag(&region, Pflag::Initialized).await;
        }

        self.bitc
            .write()
            .persist(&self.disk, region.bitc_addr, root_key)
            .await?;
        self.data_svt
            .write()
            .persist(&self.disk, region.data_svt_addr, root_key)
            .await?;
        self.index_svt
            .write()
            .persist(&self.disk, region.index_svt_addr, root_key)
            .await?;
        self.dst
            .write()
            .persist(&self.disk, region.dst_addr, root_key)
            .await?;
        self.rit.write().await.persist().await?;
        self.key_table
            .persist(&self.disk, region.keytable_addr, root_key)
            .await?;

        self.commit_pflag(&region, Pflag::Committed).await?;
        Ok(())
    }

    pub async fn load(disk: &DiskView, superblock: &SuperBlock, root_key: &Key) -> Result<Self> {
        let region = &superblock.checkpoint_region;
        match Self::check_pflag(disk, region).await {
            Pflag::NotCommitted => return_errno!(EINVAL, "checkpoint region not persisted yet"),
            Pflag::Initialized => return Ok(Self::new(superblock, disk.clone(), root_key)),
            Pflag::Committed => {}
        }

        let bitc = BITC::load(disk, region.bitc_addr, root_key).await?;
        bitc.init_bit_caches(disk).await?;
        let data_svt = SVT::load(disk, region.data_svt_addr, root_key).await?;
        let index_svt = SVT::load(disk, region.index_svt_addr, root_key).await?;
        let dst = DST::load(disk, region.dst_addr, root_key).await?;
        let rit = RIT::load(disk, region.rit_addr, superblock.data_region_addr, root_key).await?;
        let key_table = KeyTable::load(disk, region.keytable_addr, root_key).await?;

        Ok(Self {
            bitc: RwLock::new(bitc),
            data_svt: RwLock::new(data_svt),
            index_svt: RwLock::new(index_svt),
            dst: RwLock::new(dst),
            rit: AsyncRwLock::new(rit),
            key_table,
            disk: disk.clone(),
        })
    }

    async fn check_pflag(disk: &DiskView, region: &CheckpointRegion) -> Pflag {
        let mut pflag_buf = [0u8; BLOCK_SIZE];
        disk.read(region.region_addr, &mut pflag_buf).await.unwrap();
        Pflag::check_pflag(&pflag_buf)
    }

    async fn commit_pflag(&self, region: &CheckpointRegion, pflag: Pflag) -> Result<()> {
        let mut pflag_buf = [0u8; BLOCK_SIZE];
        Pflag::commit_pflag(pflag, &mut pflag_buf);
        self.disk
            .write(region.region_addr, &pflag_buf)
            .await
            .map(|_| ())
    }

    fn still_initialized(&self) -> bool {
        self.bitc().read().max_bit_version() == 0
    }
}

/// Persist flag.
#[derive(Clone, Copy, Debug)]
enum Pflag {
    NotCommitted,
    Committed,
    Initialized,
}

impl Pflag {
    fn check_pflag(pflag_buf: &[u8]) -> Pflag {
        debug_assert!(pflag_buf.len() == BLOCK_SIZE);
        if pflag_buf == &[Pflag::Committed as u8; BLOCK_SIZE] {
            Pflag::Committed
        } else if pflag_buf == &[Pflag::Initialized as u8; BLOCK_SIZE] {
            Pflag::Initialized
        } else {
            Pflag::NotCommitted
        }
    }

    fn commit_pflag(pflag: Pflag, pflag_buf: &mut [u8]) {
        debug_assert!(pflag_buf.len() == BLOCK_SIZE);
        match pflag {
            Pflag::Committed => pflag_buf.copy_from_slice(&[Pflag::Committed as u8; BLOCK_SIZE]),
            Pflag::Initialized => {
                pflag_buf.copy_from_slice(&[Pflag::Initialized as u8; BLOCK_SIZE])
            }
            _ => {}
        }
    }
}

/// Implement `persist()` and `load()` for checkpoint structures.
#[macro_export]
macro_rules! persist_load_checkpoint_region {
    ($target_struct:ident) => {
        use $crate::util::cryption::{CipherMeta, Cryption, DefaultCryptor};

        impl $target_struct {
            pub async fn persist(
                &self,
                disk: &DiskView,
                region_addr: Hba,
                root_key: &Key,
            ) -> Result<()> {
                let mut encoded_struct = Vec::new();
                self.encode(&mut encoded_struct)?;
                let bytes_len = encoded_struct.len();

                let mut cipher_buf = unsafe { Box::new_uninit_slice(bytes_len).assume_init() };
                let cipher_meta = DefaultCryptor::encrypt_arbitrary_aead(
                    &encoded_struct,
                    &mut cipher_buf,
                    root_key,
                );

                let buf_len = align_up((AUTH_ENC_MAC_SIZE + USIZE_SIZE + bytes_len), BLOCK_SIZE);
                let mut persisted_buf = Vec::with_capacity(buf_len);
                persisted_buf.extend_from_slice(cipher_meta.mac());
                persisted_buf.extend_from_slice(&bytes_len.to_le_bytes());
                persisted_buf.extend(cipher_buf.iter());
                persisted_buf.resize_with(buf_len, || 0u8);

                disk.write(region_addr, &persisted_buf).await?;
                Ok(())
            }

            pub async fn load(disk: &DiskView, region_addr: Hba, root_key: &Key) -> Result<Self> {
                let mut rbuf = [0u8; BLOCK_SIZE];
                disk.read(region_addr, &mut rbuf).await?;

                let cipher_size =
                    usize::decode(&rbuf[AUTH_ENC_MAC_SIZE..AUTH_ENC_MAC_SIZE + USIZE_SIZE])?;
                let mac: Mac = rbuf[0..AUTH_ENC_MAC_SIZE].try_into().unwrap();

                let mut cipher_buf = unsafe {
                    Box::new_uninit_slice(align_up(
                        AUTH_ENC_MAC_SIZE + USIZE_SIZE + cipher_size,
                        BLOCK_SIZE,
                    ))
                    .assume_init()
                };
                disk.read(region_addr, &mut cipher_buf).await?;
                let mut struct_buf = unsafe { Box::new_uninit_slice(cipher_size).assume_init() };
                DefaultCryptor::decrypt_arbitrary_aead(
                    &cipher_buf[AUTH_ENC_MAC_SIZE + USIZE_SIZE
                        ..AUTH_ENC_MAC_SIZE + USIZE_SIZE + cipher_size],
                    &mut struct_buf,
                    root_key,
                    &CipherMeta::new(mac),
                )?;

                $target_struct::decode(&struct_buf)
            }
        }
    };
}
// Issue: Can we use crate `serde` to serialize `Checkpoint`?

impl Debug for Checkpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Checkpoint")
            .field("BITC", &self.bitc.read())
            .field("Data_SVT", &self.data_svt.read())
            .field("Index_SVT", &self.index_svt.read())
            .field("DST", &self.dst.read())
            .field("RIT", &())
            .field("KeyTable", &self.key_table)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use block_device::mem_disk::MemDisk;

    #[test]
    fn test_checkpoint_persist_load() -> Result<()> {
        async_rt::task::block_on(async move {
            let total_blocks = 2 * GiB / BLOCK_SIZE;
            let disk = Arc::new(MemDisk::new(total_blocks).unwrap());
            let disk = DiskView::new_unchecked(disk);
            let root_key = DefaultCryptor::gen_random_key();
            let sb = SuperBlock::init(total_blocks);
            let checkpoint = Checkpoint::new(&sb, disk.clone(), &root_key);
            checkpoint.persist(&sb, &root_key).await?;
            let loaded_checkpoint = Checkpoint::load(&disk, &sb, &root_key).await?;

            assert_eq!(
                format!("{:?}", checkpoint),
                format!("{:?}", loaded_checkpoint)
            );
            Ok(())
        })
    }
}
