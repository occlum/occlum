//! Data segment cleaning (garbage collection) policy.
use crate::prelude::*;
use crate::{Checkpoint, DataCache, LsmTree, Record};

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

// Cleaner. Used to execute different cleaning policy.
#[derive(Clone)]
pub struct Cleaner(Arc<Inner>);

struct Inner {
    disk: DiskView,
    a_lock: AsyncMutex<()>,
    is_dropped: AtomicBool,
    migrants: RwLock<HashMap<Lba, Record>>,
    checkpoint: Arc<Checkpoint>,
    lsm_tree: Arc<LsmTree>,
    data_cache: Arc<DataCache>,
}

impl Cleaner {
    pub fn new(
        disk: DiskView,
        checkpoint: Arc<Checkpoint>,
        lsm_tree: Arc<LsmTree>,
        data_cache: Arc<DataCache>,
    ) -> Self {
        let new_self = Self(Arc::new(Inner {
            disk,
            a_lock: AsyncMutex::new(()),
            is_dropped: AtomicBool::new(false),
            migrants: RwLock::new(HashMap::new()),
            checkpoint,
            lsm_tree,
            data_cache,
        }));

        // Start background task
        new_self.spawn_cleaning_task();

        new_self
    }

    /// Spawn a background cleaning task.
    pub fn spawn_cleaning_task(&self) {
        let this = self.0.clone();
        // Spawn the background cleaning task
        async_rt::task::spawn(async move {
            let waiter = Waiter::new();
            loop {
                // Task exit condition
                if this.is_dropped.load(Ordering::Relaxed) {
                    break;
                }

                // Wait until timeout is reached
                let mut timeout = GC_BACKGROUND_PERIOD;
                let _ = waiter.wait_timeout(Some(&mut timeout)).await;

                this.exec_background_cleaning().await.unwrap();
            }
        });
    }

    /// Do foreground cleaning.
    #[allow(unused)]
    pub async fn exec_foreground_cleaning(&self) -> Result<()> {
        self.0.exec_foreground_cleaning().await
    }

    /// Whether cleaning is needed given a threshold.
    #[allow(unused)]
    pub fn need_cleaning(&self, threshold: usize) -> bool {
        let data_svt = self.0.checkpoint.data_svt().read();
        // Check remaining free segment
        data_svt.num_segments() - data_svt.num_allocated() <= threshold
    }

    pub fn find_migrant(&self, lba: Lba) -> Option<Record> {
        self.0.find_migrant(lba)
    }
}

impl Inner {
    async fn exec_foreground_cleaning(&self) -> Result<()> {
        let guard = self.a_lock.lock().await;

        let victim = self
            .pick_victim(NUM_BLOCKS_PER_SEGMENT)
            .await
            .ok_or(errno!(EINVAL, "cannot pick a victim"))?;
        let block_cnt = self.exec_cleaning(&victim).await?;
        debug!(
            "\n[Foreground Cleaning] complete. GC migrated {} blocks number, freed segment: {:?}\n",
            block_cnt,
            victim.segment_addr()
        );

        drop(guard);
        Ok(())
    }

    async fn exec_background_cleaning(&self) -> Result<()> {
        let guard = self.a_lock.lock().await;
        self.banish_migrants();

        let mut block_cnt = 0usize;
        let mut seg_cnt = 0usize;
        let threshold = NUM_BLOCKS_PER_SEGMENT * 5 / 8;
        for _ in 0..GC_WATERMARK {
            let victim = self.pick_victim(threshold).await;
            if victim.is_some() {
                seg_cnt += 1;
                block_cnt += self.exec_cleaning(victim.as_ref().unwrap()).await?;
            }
        }

        debug!(
            "\n[Background Cleaning] complete. GC migrated {} blocks, freed {} segments\n",
            block_cnt, seg_cnt
        );
        drop(guard);
        Ok(())
    }

    /// Concrete cleaning logic. On success, return
    /// the number of migrated blocks.
    async fn exec_cleaning(&self, victim: &Victim) -> Result<usize> {
        // Search records from index by lbas
        for &(lba, hba) in victim.valid_blocks() {
            // Check if exists newer record
            let record = self
                .lsm_tree
                .search_or_insert(lba, |searched_record| {
                    if searched_record.unwrap().hba() == hba {
                        // Insert a negative record, avoid double invalidation
                        return Some(Record::new_negative(lba));
                    }
                    None
                })
                .await?
                .unwrap();

            if record.hba() != hba {
                // Newer data are written so we just ignore current block
                continue;
            }

            self.migrants.write().insert(record.lba(), record.clone());

            // Read and decrypt from disk
            let mut rbuf = [0u8; BLOCK_SIZE];
            self.disk.read(record.hba(), &mut rbuf).await?;
            let decrypted = DefaultCryptor::decrypt_block_aead(
                &rbuf,
                &self.checkpoint.key_table().fetch_key(record.hba()),
                record.cipher_meta(),
            )?;
            rbuf.copy_from_slice(&decrypted);

            // Migrate still-valid ones by inserting back into data cache
            self.data_cache.search_or_insert(lba, &rbuf).await?;
        }

        // Validate the segment from `DST`
        self.checkpoint
            .dst()
            .write()
            .validate_or_insert(victim.segment_addr());

        // Validate the segment from `data SVT`
        self.checkpoint
            .data_svt()
            .write()
            .validate_seg(victim.segment_addr());

        Ok(victim.valid_blocks().len())
    }

    async fn pick_victim(&self, threshold: usize) -> Option<Victim> {
        let mut dst = self.checkpoint.dst().write();
        // Pick a victim segment from `DST`
        let victim_seg = dst.pick_victim()?;
        if victim_seg.valid_blocks().len() > threshold {
            return None;
        }

        // Get valid blocks' lbas from `RIT`
        let mut valid_blocks: Vec<(Lba, Hba)> = Vec::with_capacity(victim_seg.valid_blocks().len());
        let mut rit = self.checkpoint.rit().write().await;
        for &block_hba in victim_seg.valid_blocks() {
            // Get and invalidate lba in `RIT`, avoid false block invalidation(during compaction)
            // This step can ensure the victim segment can be fully freed
            let existed_lba = rit.find_and_invalidate(block_hba).await.unwrap();
            if existed_lba != NEGATIVE_LBA {
                valid_blocks.push((existed_lba, block_hba));
            }
        }

        drop(rit);
        drop(dst);
        Some(Victim::new(victim_seg.segment_addr(), valid_blocks))
    }

    fn find_migrant(&self, lba: Lba) -> Option<Record> {
        self.migrants.read().get(&lba).map(|r| r.clone())
    }

    fn banish_migrants(&self) {
        self.migrants.write().clear();
    }
}

impl Drop for Cleaner {
    fn drop(&mut self) {
        self.0.is_dropped.store(true, Ordering::Relaxed);
    }
}

#[derive(Clone, Debug)]
struct Victim {
    segment_addr: Hba,
    valid_blocks: Vec<(Lba, Hba)>,
}

impl Victim {
    pub fn new(segment_addr: Hba, valid_blocks: Vec<(Lba, Hba)>) -> Self {
        Self {
            segment_addr,
            valid_blocks,
        }
    }

    pub fn segment_addr(&self) -> Hba {
        self.segment_addr
    }

    pub fn valid_blocks(&self) -> &Vec<(Lba, Hba)> {
        &self.valid_blocks
    }
}
