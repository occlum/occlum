//! Cryption Key Table for data segments.
use crate::prelude::*;

use std::collections::HashMap;
use std::convert::TryInto;
use std::fmt::{self, Debug};

/// Cryption key table.
/// Manage per-segment cryption keys.
pub struct KeyTable {
    data_region_addr: Hba,
    table: RwLock<HashMap<Hba, Key>>,
}

impl KeyTable {
    pub fn new(data_region_addr: Hba) -> Self {
        Self {
            data_region_addr,
            table: RwLock::new(HashMap::new()),
        }
    }

    pub fn from(data_region_addr: Hba, table: HashMap<Hba, Key>) -> Self {
        Self {
            data_region_addr,
            table: RwLock::new(table),
        }
    }

    pub fn fetch_key(&self, block_addr: Hba) -> Key {
        self.table
            .write()
            .entry(self.calc_seg_addr(block_addr))
            .or_insert(DefaultCryptor::gen_random_key())
            .clone()
    }

    fn calc_seg_addr(&self, block_addr: Hba) -> Hba {
        Hba::new(align_down(
            (block_addr - self.data_region_addr.to_raw()).to_raw() as _,
            NUM_BLOCKS_PER_SEGMENT,
        ) as _)
            + self.data_region_addr.to_raw()
    }

    pub fn size(&self) -> usize {
        self.table.read().len()
    }

    /// Calculate space cost on disk.
    pub fn calc_size_on_disk(num_data_segments: usize) -> usize {
        let size = USIZE_SIZE
            + num_data_segments * (BA_SIZE + AUTH_ENC_KEY_SIZE)
            + AUTH_ENC_MAC_SIZE
            + USIZE_SIZE;
        align_up(size, BLOCK_SIZE)
    }
}

impl Serialize for KeyTable {
    fn encode(&self, encoder: &mut impl Encoder) -> Result<()> {
        self.data_region_addr.encode(encoder)?;
        self.table.read().encode(encoder)
    }

    fn decode(buf: &[u8]) -> Result<Self>
    where
        Self: Sized,
    {
        let mut offset = 0;
        let data_region_addr = Hba::decode(&buf[offset..offset + BA_SIZE])?;
        offset += BA_SIZE;
        let table = HashMap::<Hba, Key>::decode(&buf[offset..])?;

        Ok(KeyTable::from(data_region_addr, table))
    }
}

crate::impl_default_serialize! {Key, AUTH_ENC_KEY_SIZE}
crate::persist_load_checkpoint_region! {KeyTable}

impl Debug for KeyTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Checkpoint::KeyTable")
            .field("data_region_addr", &self.data_region_addr)
            .field("table_size", &self.size())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use block_device::mem_disk::MemDisk;

    #[test]
    fn test_keytable_serialize() {
        let data_region_addr = Hba::new(1);
        let key_table = KeyTable::new(data_region_addr);
        let b1 = data_region_addr + 1 as _;
        let k1 = key_table.fetch_key(b1);
        let b2 = data_region_addr + NUM_BLOCKS_PER_SEGMENT as _ + 1 as _;
        let k2 = key_table.fetch_key(b2);

        let mut bytes = Vec::new();
        key_table.encode(&mut bytes).unwrap();
        let decoded_keytable = KeyTable::decode(&bytes).unwrap();

        assert_eq!(decoded_keytable.fetch_key(b1 + 1 as _), k1);
        assert_eq!(decoded_keytable.fetch_key(b2 - 1 as _), k2);
        assert_eq!(decoded_keytable.size(), 2);
    }

    #[test]
    fn test_keytable_persist_load() -> Result<()> {
        async_rt::task::block_on(async move {
            let key_table = KeyTable::new(Hba::new(0));
            let disk = Arc::new(MemDisk::new(1024usize).unwrap());
            let disk = DiskView::new_unchecked(disk);
            let root_key = DefaultCryptor::gen_random_key();

            key_table.persist(&disk, Hba::new(0), &root_key).await?;
            let loaded_keytable = KeyTable::load(&disk, Hba::new(0), &root_key).await?;

            assert_eq!(format!("{:?}", key_table), format!("{:?}", loaded_keytable));
            Ok(())
        })
    }
}
