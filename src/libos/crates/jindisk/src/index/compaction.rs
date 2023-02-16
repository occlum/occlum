//! Lsm tree compaction policy.
use super::bit::{Bit, BitBuilder, BitId};
use super::mem_table::LockedMemTable;
use crate::index::lsm_tree::LsmLevel;
use crate::prelude::*;
use crate::{Checkpoint, Record};

use std::collections::{BTreeMap, HashSet};

/// Compactor. Used to execute different compaction policy.
pub struct Compactor(AsyncMutex<()>);

impl Compactor {
    pub fn new() -> Self {
        Self(AsyncMutex::new(()))
    }

    /// Minor compaction. (immutable MemTable -> L0)
    pub async fn exec_minor_compaction(
        &self,
        immut_mem_table: Arc<LockedMemTable>,
        disk: DiskView,
        checkpoint: Arc<Checkpoint>,
        complete_fn: impl Fn(),
    ) -> Result<()> {
        let guard = self.0.lock().await;

        if immut_mem_table.is_empty() {
            drop(guard);
            return Ok(());
        }

        let mut records = immut_mem_table.collect_all_records().await;
        assert!(
            !records.is_empty() && records.len() <= MAX_MEM_TABLE_CAPACITY,
            "Compaction error, wrong memtable size [{}]",
            records.len()
        );
        debug_assert!(records.is_sorted_by_key(|record| record.lba()));

        let l0_bit = checkpoint.bitc().write().l0_bit();

        // First check if major compaction is needed
        if l0_bit.is_some() {
            self.exec_major_compaction(l0_bit.as_ref().unwrap(), &disk, &checkpoint)
                .await?;
        }

        // Pick an available segment from `index SVT`
        let seg_addr = checkpoint.index_svt().write().pick_avail_seg().unwrap();

        // TODO: Optimize this padding logic
        Record::padding_records(&mut records, MAX_MEM_TABLE_CAPACITY);
        let level = 0 as LsmLevel;
        // Build a new BIT
        let bit = BitBuilder::new(seg_addr)
            .build(
                &records,
                &disk,
                level,
                checkpoint.bitc().write().assign_version(),
            )
            .await?;

        // Update level 0 `BITC`
        let l0_vacant = checkpoint.bitc().write().insert_bit(bit, level);
        debug_assert!(l0_vacant.is_none());

        debug!(
            "{:#?}\n[Minor Compaction] complete",
            checkpoint.bitc().read()
        );

        // Clear immutable MemTable and complete compaction
        immut_mem_table.clear().await;

        drop(guard);
        complete_fn();
        Ok(())
    }

    /// Major compaction. (Li -> Li+1)
    /// Currently L0 -> L1.
    async fn exec_major_compaction(
        &self,
        l0_bit: &Bit,
        disk: &DiskView,
        checkpoint: &Arc<Checkpoint>,
    ) -> Result<()> {
        // Find L0's overlapped bit in L1
        let mut overlapped_bits = checkpoint
            .bitc()
            .read()
            .find_bit_by_lba_range(l0_bit.lba_range(), 1 as LsmLevel);
        // Sort by version in descending order
        overlapped_bits.sort_by(|a, b| b.version().cmp(&a.version()));

        if overlapped_bits.is_empty() {
            // No overlap between L0 and L1. Just turn L0 BIT into a L1 one
            let level = 1 as LsmLevel;
            let mut bitc = checkpoint.bitc().write();
            bitc.insert_bit(l0_bit.clone(), level);
            bitc.remove_bit(l0_bit.id(), 0 as LsmLevel);
            drop(bitc);

            debug!(
                "{:#?}\n[Major Compaction] complete",
                checkpoint.bitc().read()
            );
            return Ok(());
        }
        let mut overlapped_bit_ids: Vec<BitId> = Vec::with_capacity(overlapped_bits.len());

        let l0_records = l0_bit.collect_all_records(disk).await?;

        // Construct L0 regular records map and collect negative ones
        let mut records_merge_map = BTreeMap::<Lba, Record>::new();
        let mut negative_map = HashSet::<Lba>::new();
        for l0_record in l0_records.into_iter() {
            if l0_record.is_negative() {
                negative_map.insert(l0_record.lba());
            } else {
                records_merge_map.insert(l0_record.lba(), l0_record);
            }
        }

        // Collect L1 bits from newer to older
        for l1_bit in overlapped_bits {
            let l1_records = l1_bit.collect_all_records(disk).await?;
            for l1_record in l1_records.into_iter() {
                let lba = l1_record.lba();
                if l1_record.is_negative() {
                    negative_map.insert(lba);
                    continue;
                }
                if negative_map.contains(&lba) {
                    continue;
                }
                if records_merge_map.contains_key(&lba) {
                    // Check if the hba has already been processed by cleaning
                    if checkpoint
                        .rit()
                        .write()
                        .check_valid(l1_record.hba(), lba)
                        .await
                    {
                        // Delayed block reclamation
                        checkpoint
                            .dst()
                            .write()
                            .update_validity(&[l1_record.hba()], false);
                    }
                    continue;
                }
                records_merge_map.insert(lba, l1_record);
            }
            overlapped_bit_ids.push(*l1_bit.id());
        }

        // All wait-compaction records
        let mut records_merged: Vec<Record> = records_merge_map.into_values().collect();
        debug_assert!(records_merged.is_sorted_by_key(|record| record.lba()));

        // TODO: Optimize this padding logic
        Record::padding_records(&mut records_merged, MAX_MEM_TABLE_CAPACITY);
        // Construct several new BITs
        for sub_records in records_merged.chunks(MAX_MEM_TABLE_CAPACITY) {
            // Pick an available segment from index SVT
            let seg_addr = checkpoint.index_svt().write().pick_avail_seg().unwrap();

            let level = 1 as LsmLevel;
            // Build a new BIT
            let bit = BitBuilder::new(seg_addr)
                .build(
                    sub_records,
                    disk,
                    level,
                    checkpoint.bitc().write().assign_version(),
                )
                .await?;

            // Insert new one in BITC
            checkpoint.bitc().write().insert_bit(bit, level);
        }

        // TODO: Make below steps atomic
        let mut bitc = checkpoint.bitc().write();
        let mut index_svt = checkpoint.index_svt().write();
        for bit_id in overlapped_bit_ids {
            // Remove old one in BITC
            bitc.remove_bit(&bit_id, 1 as LsmLevel);

            // Validate the index segment in SVT
            index_svt.validate_seg(bit_id);
        }
        // Remove l0 BIT and validate the segment
        bitc.remove_bit(l0_bit.id(), 0 as LsmLevel);
        index_svt.validate_seg(*l0_bit.id());
        drop(bitc);
        drop(index_svt);

        debug!(
            "{:#?}\n[Major Compaction] complete",
            checkpoint.bitc().read()
        );
        Ok(())
    }
}
