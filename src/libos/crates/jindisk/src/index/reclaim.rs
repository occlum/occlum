//! Block reclaim policy.
use super::mem_table::MemRecord;
use crate::prelude::*;
use crate::{Bit, Checkpoint, Record};

use std::collections::{BTreeMap, HashSet};
use std::future::{Future, IntoFuture};
use std::pin::Pin;

/// Delayed block reclamation policy in memtable.
///
/// Currently, A `Negative` is insert into memtable only when GC(garbage collection) started and check a Hba is still valid (no newer data).
/// `Regular` and `Negative` are easy to understand. But `NegativeThenRegular` here always means the regular one is newer coming than the negative one.
/// We only reclaim a Hba when `Regular` meets a `Regular`.
/// When a `Negative` arrives, all older ones can be deprecated (they are already reclaimed and are now processed by GC)
/// The `Negative` ones cannot be removed directly in memtable since they are needed in compaction.
pub fn apply_memtable_reclaim_policy<'a>(
    checkpoint: Arc<Checkpoint>,
    coming_record: &'a Record,
    existed_record: &'a MemRecord,
) -> Pin<Box<dyn Future<Output = MemRecord> + Send + 'a>> {
    async fn inner(
        checkpoint: Arc<Checkpoint>,
        coming_record: &Record,
        existed_record: &MemRecord,
    ) -> MemRecord {
        debug_assert!(coming_record.lba() == existed_record.lba());
        let target_lba = coming_record.lba();
        let mut reclaimed_hba = None;

        let to_be_replaced = match (coming_record.is_negative(), existed_record) {
            (false, MemRecord::Regular(r)) => {
                let _ = reclaimed_hba.insert(r.hba());
                MemRecord::Regular(coming_record.clone())
            }
            (false, MemRecord::Negative(_)) => {
                MemRecord::NegativeThenRegular(coming_record.clone())
            }
            (false, MemRecord::NegativeThenRegular(r)) => {
                let _ = reclaimed_hba.insert(r.hba());
                MemRecord::NegativeThenRegular(coming_record.clone())
            }
            (true, MemRecord::Regular(_)) => MemRecord::Negative(target_lba),
            (true, MemRecord::NegativeThenRegular(_)) => MemRecord::Negative(target_lba),
            (_, _) => {
                panic!("should not happen");
            }
        };

        if let Some(reclaimed_hba) = reclaimed_hba {
            // Check if the hba has already been processed by cleaning
            let avail_to_reclaim = checkpoint
                .rit()
                .write()
                .await
                .check_valid(reclaimed_hba, target_lba)
                .await;
            if avail_to_reclaim {
                // Delayed block reclamation
                checkpoint
                    .dst()
                    .write()
                    .update_validity(&[reclaimed_hba], false);
            }
        }

        to_be_replaced
    }

    Box::pin(inner(checkpoint, coming_record, existed_record).into_future())
}

/// Delayed block reclamation policy in compaction.
///
/// When older records meet newer records (same lba) during compaction (currently L0 -> L1),
/// 1. If there exists a newer negative record (which means a newer write occurred), older ones should be discarded.
/// 2. If There is no newer negative record,
///   2.a. If the hba of older record's corresponding lba in `RIT` is negative, do not reclaim since this block has already been processed by cleaning.
///   2.b. If not, then do delayed block reclamation: mark the hba of older record **Invalid** in `DST`, so it can be dealt with in later cleaning.
pub async fn apply_compaction_reclaim_policy(
    l0_bit: &Bit,
    overlapped_l1_bits: &Vec<Bit>,
    disk: &DiskView,
    checkpoint: &Arc<Checkpoint>,
) -> Result<Vec<Record>> {
    debug_assert!(
        !overlapped_l1_bits.is_empty()
            && overlapped_l1_bits
                .is_sorted_by(|bit1, bit2| Some(bit2.version().cmp(&bit1.version())))
    );

    // Construct L0 regular records map and collect negative ones
    let (mut compacted_records_map, mut negative_map) = {
        let mut compacted_records_map = BTreeMap::<Lba, Record>::new();
        let mut negative_map = HashSet::<Lba>::new();

        let l0_records = l0_bit.collect_all_records(disk).await?;
        for l0_record in l0_records.into_iter() {
            if l0_record.is_negative() {
                negative_map.insert(l0_record.lba());
            } else {
                compacted_records_map.insert(l0_record.lba(), l0_record);
            }
        }
        (compacted_records_map, negative_map)
    };

    // Scan each overlapped L1 BIT and apply reclaim policy
    for l1_bit in overlapped_l1_bits.iter() {
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
            if compacted_records_map.contains_key(&lba) {
                // Check if the hba has already been processed by cleaning
                let avail_to_reclaim = checkpoint
                    .rit()
                    .write()
                    .await
                    .check_valid(l1_record.hba(), lba)
                    .await;
                if avail_to_reclaim {
                    // Delayed block reclamation
                    checkpoint
                        .dst()
                        .write()
                        .update_validity(&[l1_record.hba()], false);
                }
                continue;
            }
            compacted_records_map.insert(lba, l1_record);
        }
    }

    Ok(compacted_records_map.into_values().collect())
}
