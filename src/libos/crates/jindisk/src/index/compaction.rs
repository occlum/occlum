//! Lsm tree compaction policy.
use super::bit::{Bit, BitBuilder, MAX_RECORD_NUM_PER_BIT};
use super::mem_table::LockedMemTable;
use super::reclaim::apply_compaction_reclaim_policy;
use crate::index::lsm_tree::LsmLevel;
use crate::prelude::*;
use crate::{Checkpoint, Record};

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
        Record::padding_records(&mut records, MAX_RECORD_NUM_PER_BIT);
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
        // Find L0's overlapped BIT in L1
        let overlapped_l1_bits = {
            let mut overlapped_bits = checkpoint
                .bitc()
                .read()
                .find_bit_by_lba_range(l0_bit.lba_range(), 1 as LsmLevel);

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

            // Sort by version in descending order (newer to older)
            overlapped_bits.sort_by(|bit1, bit2| bit2.version().cmp(&bit1.version()));
            overlapped_bits
        };

        // All wait-compaction records
        let compacted_records = {
            let mut records =
                apply_compaction_reclaim_policy(l0_bit, &overlapped_l1_bits, disk, checkpoint)
                    .await?;
            debug_assert!(records.is_sorted_by_key(|r| r.lba()));
            // TODO: Optimize this padding logic
            Record::padding_records(&mut records, MAX_RECORD_NUM_PER_BIT);
            records
        };

        // Construct several new BITs
        let mut new_bits = Vec::with_capacity(compacted_records.len() / MAX_RECORD_NUM_PER_BIT);
        for sub_records in compacted_records.chunks(MAX_RECORD_NUM_PER_BIT) {
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

            new_bits.push(bit);
        }

        let mut bitc = checkpoint.bitc().write();
        // Insert new one in BITC
        for new_bit in new_bits {
            let level = new_bit.level();
            bitc.insert_bit(new_bit, level);
        }
        let mut index_svt = checkpoint.index_svt().write();
        overlapped_l1_bits.iter().for_each(|bit| {
            let bit_id = bit.id();
            // Remove old one in BITC
            bitc.remove_bit(bit_id, 1 as LsmLevel);

            // Validate the index segment in SVT
            index_svt.validate_seg(bit_id);
        });
        // Remove l0 BIT and validate the segment
        bitc.remove_bit(l0_bit.id(), 0 as LsmLevel);
        index_svt.validate_seg(l0_bit.id());
        drop(index_svt);
        drop(bitc);

        debug!(
            "{:#?}\n[Major Compaction] complete",
            checkpoint.bitc().read()
        );
        Ok(())
    }
}
