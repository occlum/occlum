//! Index structure (Log-structured merged tree).
//!
//! A disk-oriented secure LSM-tree to organize the disk components
//! (referred to as Block Index Tables) directly on raw disk.
//!
//! **LSM Tree architecture:**
//! ```text
//!              |MemTable|
//!         |Immutable MemTable|               memory
//! ---------------------------------------------------
//!             |BIT|              | Level 0     disk
//!  |BIT| |BIT| |BIT| ... |BIT|   | Level 1
//! ```
use super::compaction::Compactor;
use super::mem_table::LockedMemTable;
use super::reclaim::*;
use crate::prelude::*;
use crate::util::RangeQueryCtx;
use crate::{Checkpoint, Record};

use std::fmt::{self, Debug};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Log-structured merged tree.
pub struct LsmTree {
    level: LsmLevel,
    mem_tables: [Arc<LockedMemTable>; 2],
    immut_idx: AtomicU8,
    compactor: Arc<Compactor>,
    disk: DiskView,
    checkpoint: Arc<Checkpoint>,
    wq: Arc<WaiterQueue>,
    arw_lock: AsyncRwLock<()>,
}
// TODO: Make MemTable/BIT capacity configurable

pub type LsmLevel = u8;

impl LsmTree {
    /// Initialize a `LsmTree` given a start hba.
    pub fn new(disk: DiskView, checkpoint: Arc<Checkpoint>) -> Self {
        // MemTable
        let mut mem_table = LockedMemTable::new(MAX_MEM_TABLE_CAPACITY);
        // Immutable MemTable
        let mut immut_mem_table = LockedMemTable::new(MAX_MEM_TABLE_CAPACITY);

        // Register reclaim callback
        let cloned_ckpt = checkpoint.clone();
        mem_table.register_reclaim_callback(Box::new(move |new, old| {
            apply_memtable_reclaim_policy(cloned_ckpt.clone(), new, old)
        }));
        let cloned_ckpt = checkpoint.clone();
        immut_mem_table.register_reclaim_callback(Box::new(move |new, old| {
            apply_memtable_reclaim_policy(cloned_ckpt.clone(), new, old)
        }));

        let mem_tables = [Arc::new(mem_table), Arc::new(immut_mem_table)];

        const MAX_LSM_LEVEL: LsmLevel = 2;
        Self {
            level: MAX_LSM_LEVEL,
            mem_tables,
            immut_idx: AtomicU8::new(1),
            compactor: Arc::new(Compactor::new()),
            disk,
            checkpoint,
            wq: Arc::new(WaiterQueue::new()),
            arw_lock: AsyncRwLock::new(()),
        }
    }

    /// Insert a record into lsm tree.
    pub async fn insert(&self, lba: Lba, record: Record) -> Result<()> {
        let aw_lock = self.arw_lock.write().await;

        self.insert_record(lba, record).await?;

        drop(aw_lock);
        Ok(())
    }

    /// Search a record from lsm tree.
    pub async fn search(&self, target_lba: Lba) -> Option<Record> {
        let ar_lock = self.arw_lock.read().await;

        let result_record = self.search_record(target_lba).await;

        drop(ar_lock);
        result_record
    }

    /// Concrete insert logic.
    async fn insert_record(&self, lba: Lba, record: Record) -> Result<()> {
        let mem_table = self.mem_table();

        self.wait_compaction(mem_table).await?;

        let is_full = mem_table.insert(lba, record).await.unwrap();

        // If MemTable is full, trigger minor compaction
        // Immutable MemTable -> BIT
        if is_full {
            // Turn to immutable
            self.immut_idx.fetch_xor(1, Ordering::Relaxed);

            // Background compaction
            let compactor = self.compactor.clone();
            let mem_table = mem_table.clone();
            let disk = self.disk.clone();
            let checkpoint = self.checkpoint.clone();
            let wq = self.wq.clone();
            #[cfg(feature = "sgx")]
            async_rt::task::spawn(async move {
                compactor
                    .exec_minor_compaction(mem_table, disk, checkpoint, || {
                        wq.wake_all();
                    })
                    .await
                    .unwrap();
            });
            // Foreground compaction (Test-purpose)
            #[cfg(not(feature = "sgx"))]
            compactor
                .exec_minor_compaction(mem_table, disk, checkpoint, || {
                    wq.wake_all();
                })
                .await?;
        }

        Ok(())
    }

    /// Concrete search logic.
    /// MemTable -> Immutable MemTable -> L0 BIT -> L1 BITs
    async fn search_record(&self, target_lba: Lba) -> Option<Record> {
        // Search in MemTable
        let mem_table = self.mem_table();
        if let Some(record) = mem_table.search(target_lba).await {
            return Some(record);
        }

        // Search in immutable MemTable
        let immut_mem_table = self.immut_mem_table();
        if let Some(record) = immut_mem_table.search(target_lba).await {
            return Some(record);
        }

        // Search in BITs
        // Search target BIT in BITC
        let mut result_record = None;

        // Search L0
        if let Some(l0_bit) = self
            .checkpoint
            .bitc()
            .read()
            .find_bit_by_lba(target_lba, 0 as LsmLevel)
        {
            debug_assert!(l0_bit.lba_range().is_within_range(target_lba));

            result_record = l0_bit.search(target_lba, &self.disk).await
        }

        // Search L1
        if result_record.is_none() {
            if let Some(l1_bit) = self
                .checkpoint
                .bitc()
                .read()
                .find_bit_by_lba(target_lba, 1 as LsmLevel)
            {
                debug_assert!(l1_bit.lba_range().is_within_range(target_lba));

                result_record = l1_bit.search(target_lba, &self.disk).await
            }
        }

        result_record
    }

    /// Range query.
    pub async fn search_range(&self, query_ctx: &mut RangeQueryCtx) -> Vec<Record> {
        let ar_lock = self.arw_lock.read().await;
        let mut searched_records = Vec::with_capacity(query_ctx.num_queried_blocks());

        // Search in MemTable
        let mem_table = self.mem_table();
        mem_table
            .search_range(query_ctx, &mut searched_records)
            .await;
        if query_ctx.is_completed() {
            return searched_records;
        }

        // Search in immutable MemTable
        let immut_mem_table = self.immut_mem_table();
        immut_mem_table
            .search_range(query_ctx, &mut searched_records)
            .await;
        if query_ctx.is_completed() {
            return searched_records;
        }

        // Search in BITs
        // Search target BIT in BITC

        // Search L0
        let l0_bit = self
            .checkpoint
            .bitc()
            .read()
            .find_bit_by_lba_range(query_ctx.target_range(), 0 as LsmLevel);
        {
            for bit in l0_bit {
                bit.search_range(query_ctx, &self.disk, &mut searched_records)
                    .await
            }
            if query_ctx.is_completed() {
                return searched_records;
            }
        }

        // Search L1
        let l1_bits = self
            .checkpoint
            .bitc()
            .read()
            .find_bit_by_lba_range(query_ctx.target_range(), 1);
        {
            for bit in l1_bits {
                bit.search_range(query_ctx, &self.disk, &mut searched_records)
                    .await;
            }
        }

        drop(ar_lock);
        searched_records
    }

    /// Search or insert a record into lsm tree on a given condition.
    /// Currently used for segment cleaning, must be atomic.
    pub async fn search_or_insert(
        &self,
        target_lba: Lba,
        decide_fn: impl Fn(Option<Record>) -> Option<Record>,
    ) -> Result<Option<Record>> {
        let aw_lock = self.arw_lock.write().await;

        let searched_record = self.search_record(target_lba).await;

        let to_be_inserted = decide_fn(searched_record.clone());

        if let Some(record) = to_be_inserted {
            self.insert_record(target_lba, record).await?;
        }

        drop(aw_lock);
        Ok(searched_record)
    }

    fn mem_table(&self) -> &Arc<LockedMemTable> {
        &self.mem_tables[(self.immut_idx.load(Ordering::Relaxed) as usize) ^ 1]
    }

    fn immut_mem_table(&self) -> &Arc<LockedMemTable> {
        &self.mem_tables[(self.immut_idx.load(Ordering::Relaxed) as usize)]
    }

    async fn wait_compaction(&self, mem_table: &Arc<LockedMemTable>) -> Result<()> {
        // Fast path
        if !mem_table.is_full() {
            return Ok(());
        }

        // Slow path
        let mut waiter = Waiter::new();
        self.wq.enqueue(&mut waiter);
        while mem_table.is_full() {
            let _ = waiter.wait().await;
        }
        self.wq.dequeue(&mut waiter);
        Ok(())
    }
}

impl LsmTree {
    /// Persist MemTables to BIT.
    pub async fn persist(&self) -> Result<()> {
        let aw_lock = self.arw_lock.write().await;
        // Wait immutable MemTable to finish compaction
        self.wait_compaction(self.immut_mem_table()).await?;

        let mem_table = self.mem_table();
        if mem_table.is_empty() {
            return Ok(());
        }
        self.compactor
            .exec_minor_compaction(
                mem_table.clone(),
                self.disk.clone(),
                self.checkpoint.clone(),
                || {
                    self.wq.wake_all();
                },
            )
            .await?;

        drop(aw_lock);
        Ok(())
    }
}

impl Debug for LsmTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LsmTree")
            .field("level", &self.level)
            .field("memtable_size", &self.mem_table().num_records())
            .field("immut_memtable_size", &self.immut_mem_table().num_records())
            .finish()
    }
}
