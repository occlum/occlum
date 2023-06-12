//! Disk array.
use crate::data::DataBlock;
use crate::prelude::*;

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::mem::size_of;

/// Disk array that manages on-disk structures.
pub struct DiskArray<T> {
    start_addr: Hba,
    table: BTreeMap<Hba, DataBlock>,
    disk: DiskView,
    key: Key,
    phantom: PhantomData<T>,
}

impl<T: Serialize> DiskArray<T> {
    pub fn new(start_addr: Hba, disk: DiskView, key: &Key) -> Self {
        Self {
            start_addr,
            table: BTreeMap::new(),
            disk,
            key: key.clone(),
            phantom: PhantomData,
        }
    }

    pub async fn get(&mut self, offset: usize) -> Option<T> {
        self.check_offset(offset);

        let (hba, inner_offset) = self.hba_and_inner_offset(offset);
        let data_block = self.load_block(hba).await.ok()?;

        T::decode(&data_block.as_slice()[inner_offset..(inner_offset + Self::unit_size())]).ok()
    }

    pub async fn set(&mut self, offset: usize, unit: T) -> Result<()> {
        self.check_offset(offset);

        let (hba, inner_offset) = self.hba_and_inner_offset(offset);
        let data_block = self.load_block(hba).await?;

        let mut buf = Vec::with_capacity(Self::unit_size());
        unit.encode(&mut buf)?;
        data_block.as_slice_mut()[inner_offset..(inner_offset + Self::unit_size())]
            .copy_from_slice(&buf);
        Ok(())
    }

    fn hba_and_inner_offset(&self, offset: usize) -> (Hba, usize) {
        let size = offset * Self::unit_size();
        (
            self.start_addr + Hba::from_byte_offset(align_down(size, BLOCK_SIZE)).to_raw(),
            size % BLOCK_SIZE,
        )
    }

    async fn load_block(&mut self, hba: Hba) -> Result<&mut DataBlock> {
        if !self.table.contains_key(&hba) {
            let mut data_block = DataBlock::new_uninit();
            self.disk.read(hba, data_block.as_slice_mut()).await?;

            let iv = (hba.to_raw() as u128).to_le_bytes();
            let plaintext =
                DefaultCryptor::decrypt_block(data_block.as_slice(), &self.key, Some(&iv))?;
            data_block.as_slice_mut().copy_from_slice(&plaintext);

            let _ = self.table.insert(hba, data_block);
        }

        Ok(self.table.get_mut(&hba).unwrap())
    }

    pub fn unit_size() -> usize {
        size_of::<T>()
    }

    pub fn table_size(&self) -> usize {
        self.table.len()
    }

    fn check_offset(&self, offset: usize) {
        debug_assert!(
            self.start_addr.to_offset() + offset * Self::unit_size() <= self.disk.total_bytes()
        )
    }

    pub async fn persist(&self) -> Result<()> {
        // TODO: Support batch write
        for (hba, block) in self.table.iter() {
            let iv = (hba.to_raw() as u128).to_le_bytes();
            let ciphertext = DefaultCryptor::encrypt_block(block.as_slice(), &self.key, Some(&iv))?;
            self.disk.write(*hba, &ciphertext).await?;
        }
        Ok(())
    }
}
