//! MemTable of lsm tree.
use crate::prelude::*;
use crate::util::RangeQueryCtx;
use crate::Record;
use async_rt::sync::RwLockWriteGuard;

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};

pub type BTreeMemTable = BTreeMap<Lba, MemRecord>;
// TODO: Add `RBTreeMemTable`

/// A thread-safe `MemTable`.
pub struct LockedMemTable {
    mem_table: AsyncRwLock<BTreeMemTable>,
    num_records: AtomicUsize,
    capacity: usize,
    reclaim_fn: Option<ReclaimFn>,
}

/// `Record` in memory.
#[derive(Clone, Debug)]
pub enum MemRecord {
    /// A regular record
    Regular(Record),
    /// A negative record, only its lba is meaningful
    Negative(Lba),
    /// Same lba, double records (newer is regular, older is negative)
    NegativeThenRegular(Record),
}

/// Reclaim callback function for MemTable.
pub type ReclaimFn = Box<
    dyn for<'a> Fn(
            &'a Record,
            &'a MemRecord,
        ) -> Pin<Box<dyn Future<Output = MemRecord> + Send + 'a>>
        + Send
        + Sync,
>;

impl LockedMemTable {
    pub fn new(capacity: usize) -> Self {
        Self {
            mem_table: AsyncRwLock::new(BTreeMemTable::new()),
            num_records: AtomicUsize::new(0),
            capacity,
            reclaim_fn: None,
        }
    }

    pub fn register_reclaim_callback(&mut self, reclaim_fn: ReclaimFn) {
        let _ = self.reclaim_fn.insert(reclaim_fn);
    }

    /// Insert a record into MemTable.
    pub async fn insert(&self, lba: Lba, record: Record) -> Result<bool> {
        if self.is_full() {
            return_errno!(EINVAL, "memtable is full, not available to insert");
        }

        let mut mem_table = self.mem_table.write().await;

        let to_be_inserted: MemRecord = if self.reclaim_fn.is_some() && mem_table.contains_key(&lba)
        {
            self.reclaim_fn.as_ref().unwrap()(&record, mem_table.get(&lba).unwrap()).await
        } else {
            if !record.is_negative() {
                MemRecord::Regular(record)
            } else {
                MemRecord::Negative(lba)
            }
        };

        self.insert_mem_record(&mut mem_table, lba, to_be_inserted)
            .await
    }

    async fn insert_mem_record(
        &'a self,
        mem_table: &mut RwLockWriteGuard<'a, BTreeMap<Lba, MemRecord>>,
        lba: Lba,
        mem_record: MemRecord,
    ) -> Result<bool> {
        let if_existed = mem_table.insert(lba, mem_record.clone());

        let inc_num_records = || {
            self.num_records.fetch_add(1, Ordering::Relaxed);
        };
        let dec_num_records = || {
            self.num_records.fetch_sub(1, Ordering::Relaxed);
        };
        match if_existed {
            Some(existed_record) => match (&mem_record, existed_record) {
                (
                    MemRecord::Regular(_) | MemRecord::Negative(_),
                    MemRecord::NegativeThenRegular(_),
                ) => dec_num_records(),
                (
                    MemRecord::NegativeThenRegular(_),
                    MemRecord::Regular(_) | MemRecord::Negative(_),
                ) => inc_num_records(),
                (_, _) => {}
            },
            None => match &mem_record {
                MemRecord::Regular(_) | MemRecord::Negative(_) => inc_num_records(),
                MemRecord::NegativeThenRegular(_) => {
                    for _ in 0..2 {
                        inc_num_records();
                    }
                }
            },
        }

        Ok(self.is_full())
    }

    // Search a record from MemTable.
    pub async fn search(&self, target_lba: Lba) -> Option<Record> {
        let mem_table = self.mem_table.read().await;
        if let Some(mem_record) = mem_table.get(&target_lba) {
            match mem_record {
                MemRecord::Regular(record) | MemRecord::NegativeThenRegular(record) => {
                    return Some(record.clone())
                }
                MemRecord::Negative(lba) => return Some(Record::new_negative(*lba)),
            }
        }
        None
    }

    /// Range search multi records from consecutive lbas.
    /// Searched records are collected into result vector.
    pub async fn search_range(
        &self,
        query_ctx: &mut RangeQueryCtx,
        searched_records: &mut Vec<Record>,
    ) {
        let mem_table = self.mem_table.read().await;
        let target_range = query_ctx.target_range();
        for (_, mem_record) in mem_table.range(target_range.start()..target_range.end()) {
            match mem_record {
                MemRecord::Regular(record) | MemRecord::NegativeThenRegular(record) => {
                    let lba = record.lba();
                    searched_records.push(record.clone());
                    query_ctx.complete(lba);
                }
                MemRecord::Negative(lba) => {
                    searched_records.push(Record::new_negative(*lba));
                    query_ctx.complete(*lba);
                }
            }
        }
    }

    pub async fn collect_all_records(&self) -> Vec<Record> {
        let mem_table = self.mem_table.read().await;

        let mut records = Vec::with_capacity(self.capacity);
        for (_, mem_record) in mem_table.iter() {
            match mem_record {
                MemRecord::Regular(record) => records.push(record.clone()),
                MemRecord::Negative(lba) => records.push(Record::new_negative(*lba)),
                MemRecord::NegativeThenRegular(record) => {
                    records.push(record.clone());
                    records.push(Record::new_negative(record.lba()));
                }
            }
        }

        records
    }

    #[allow(unused)]
    pub async fn iter(&self) -> std::vec::IntoIter<Record> {
        self.collect_all_records().await.into_iter()
    }

    pub async fn clear(&self) {
        let mut mem_table = self.mem_table.write().await;
        self.num_records.store(0, Ordering::Relaxed);
        mem_table.clear()
    }

    pub fn num_records(&self) -> usize {
        self.num_records.load(Ordering::Relaxed)
    }

    pub fn is_full(&self) -> bool {
        self.num_records.load(Ordering::Relaxed) >= self.capacity
    }

    pub fn is_empty(&self) -> bool {
        self.num_records.load(Ordering::Relaxed) == 0
    }
}

impl MemRecord {
    pub fn lba(&self) -> Lba {
        match self {
            MemRecord::Regular(record) => record.lba(),
            MemRecord::Negative(lba) => *lba,
            MemRecord::NegativeThenRegular(record) => record.lba(),
        }
    }
}

// Safety due to internal lock control.
unsafe impl Send for LockedMemTable {}
unsafe impl Sync for LockedMemTable {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::bit::disk_bit::MAX_RECORD_NUM_PER_BIT;

    #[test]
    fn preview_mem_table_config() {
        assert_eq!(MAX_MEM_TABLE_CAPACITY, MAX_RECORD_NUM_PER_BIT);
        println!("MAX_MEM_TABLE_CAPACITY: {}", MAX_MEM_TABLE_CAPACITY);
    }
}
