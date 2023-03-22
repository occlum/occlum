//! Block reclaim policy.
use super::mem_table::MemRecord;
use crate::prelude::*;
use crate::{Checkpoint, Record};

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

// TODO: Add reclamation policy for compaction
#[allow(unused)]
async fn apply_compaction_reclaim_policy(
    checkpoint: &Arc<Checkpoint>,
    new_record: &Record,
    old_record: &Record,
) -> Record {
    todo!()
}
