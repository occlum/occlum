//! Disk array.
use super::disk_shadow::DiskShadow;
use crate::data::DataBlock;
use crate::prelude::*;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem::size_of;

/// Disk array that manages on-disk structures.
/// Layout of one underlying block:
///   +------+-------------+------------+
///   | hmac | (unit, ...) | padding(0) |
///   +------+-------------+------------+
pub struct DiskArray<T> {
    table: HashMap<Hba, DataBlock>,
    nr_units: usize,
    disk: DiskShadow,
    key: Key,
    hmac: bool,
    phantom: PhantomData<T>,
}

impl<T: Serialize> DiskArray<T> {
    pub fn new(nr_units: usize, disk: DiskShadow, key: Key, hmac: bool) -> Self {
        Self {
            table: HashMap::new(),
            nr_units,
            disk,
            key,
            hmac,
            phantom: PhantomData,
        }
    }

    pub async fn get(&mut self, offset: usize) -> Result<T> {
        self.check_offset(offset)?;

        let (hba, inner_offset) = self.hba_and_inner_offset(offset);
        let data_block = self.load_block(hba).await?;

        T::decode(&data_block.as_slice()[inner_offset..(inner_offset + Self::unit_size())])
    }

    pub async fn set(&mut self, offset: usize, unit: T) -> Result<()> {
        self.check_offset(offset)?;

        let (hba, inner_offset) = self.hba_and_inner_offset(offset);
        let data_block = self.load_block(hba).await?;

        let mut buf = Vec::with_capacity(Self::unit_size());
        unit.encode(&mut buf)?;
        data_block.as_slice_mut()[inner_offset..(inner_offset + Self::unit_size())]
            .copy_from_slice(&buf);
        Ok(())
    }

    fn check_offset(&self, offset: usize) -> Result<()> {
        if offset >= self.nr_units {
            return_errno!(EINVAL, "Illegal offset in DiskArray");
        }
        Ok(())
    }

    fn unit_size() -> usize {
        let size = size_of::<T>();
        debug_assert!(size > 0 && size <= BLOCK_SIZE - AUTH_ENC_MAC_SIZE);
        size
    }

    fn unit_per_block() -> usize {
        (BLOCK_SIZE - AUTH_ENC_MAC_SIZE) / Self::unit_size()
    }

    fn hba_and_inner_offset(&self, offset: usize) -> (Hba, usize) {
        let unit_per_block = Self::unit_per_block();
        let mut block_offset = (offset / unit_per_block) as _;
        (
            self.disk.boundary().start() + block_offset,
            AUTH_ENC_MAC_SIZE + (offset % unit_per_block) * Self::unit_size(),
        )
    }

    async fn load_block(&mut self, hba: Hba) -> Result<&mut DataBlock> {
        if !self.table.contains_key(&hba) {
            let mut buf = [0u8; BLOCK_SIZE];
            self.disk.read(hba, &mut buf).await?;
            let mut data_block = DataBlock::new_uninit();
            // FIXME: skip decryption may cause error (corner case)
            if buf[..AUTH_ENC_MAC_SIZE].ne(&[0u8; AUTH_ENC_MAC_SIZE]) {
                if self.hmac {
                    let mut mac = [0u8; AUTH_ENC_MAC_SIZE];
                    mac.copy_from_slice(&buf[..AUTH_ENC_MAC_SIZE]);
                    let cipher_meta = CipherMeta::new(mac);
                    DefaultCryptor::decrypt_arbitrary(
                        &buf[AUTH_ENC_MAC_SIZE..],
                        &mut data_block.as_slice_mut()[AUTH_ENC_MAC_SIZE..],
                        &self.key,
                        &cipher_meta,
                    )?;
                } else {
                    let buf = DefaultCryptor::symm_decrypt_block(&buf, &self.key)?;
                    data_block.as_slice_mut().copy_from_slice(&buf);
                }
            } else {
                data_block.as_slice_mut().copy_from_slice(&buf);
            }
            self.table.insert(hba, data_block);
        }
        Ok(self.table.get_mut(&hba).unwrap())
    }

    pub fn total_blocks(nr_units: usize) -> usize {
        (nr_units + Self::unit_per_block() - 1) / Self::unit_per_block()
    }

    pub fn total_blocks_with_shadow(nr_units: usize) -> usize {
        let nr_blocks = Self::total_blocks(nr_units);
        DiskShadow::total_blocks_with_shadow(nr_blocks)
    }

    pub fn table_size(&self) -> usize {
        self.table.len()
    }

    pub async fn persist(&mut self, checkpoint: bool) -> Result<bool> {
        for (hba, block) in self.table.iter() {
            // Encrypt block with or without HMAC
            if self.hmac {
                let mut buf = [0u8; BLOCK_SIZE];
                let cipher_meta = DefaultCryptor::encrypt_arbitrary(
                    &block.as_slice()[AUTH_ENC_MAC_SIZE..],
                    &mut buf[AUTH_ENC_MAC_SIZE..],
                    &self.key,
                );
                buf[..AUTH_ENC_MAC_SIZE].copy_from_slice(cipher_meta.mac());
                self.disk.write(*hba, &buf).await?;
            } else {
                let buf = DefaultCryptor::symm_encrypt_block(block.as_slice(), &self.key)?;
                self.disk.write(*hba, &buf).await?;
            }
        }
        self.disk.persist(checkpoint).await
    }
}
