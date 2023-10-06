use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use sov_first_read_last_write_cache::cache::CacheLog;
use sov_first_read_last_write_cache::CacheKey;
use crate::db::Persistence;
use crate::rollup_interface::{ForkTreeManager, Snapshot};
use crate::state::{FrozenSnapshot, SnapshotId, TreeManagerSnapshotQuery};
use crate::types::{Key, ReadOnlyLock, Value};

pub type BlockHash = String;

pub struct BlockStateManager<P: Persistence<Payload=CacheLog>> {
    db: Arc<Mutex<P>>,
    // Chain: prev_block -> child_blocks (forks
    chain_forks: HashMap<BlockHash, Vec<BlockHash>>,
    // Reverse: child_block -> parent
    to_parent: HashMap<BlockHash, BlockHash>,
    // Snapshots
    // snapshot_id => snapshot
    snapshots: HashMap<SnapshotId, FrozenSnapshot>,
    // All ancestors of a given snapshot kid => parent
    snapshot_ancestors: HashMap<SnapshotId, SnapshotId>,

    block_hash_to_id: HashMap<BlockHash, SnapshotId>,

    // Incremental
    latest_id: SnapshotId,

    self_ref: Option<Arc<RwLock<BlockStateManager<P>>>>,
}

impl<P: Persistence<Payload=CacheLog>> BlockStateManager<P> {
    pub fn new_locked(db: Arc<Mutex<P>>) -> Arc<RwLock<Self>> {
        let block_state_manager = Arc::new(RwLock::new(Self {
            db,
            chain_forks: Default::default(),
            to_parent: Default::default(),
            snapshots: Default::default(),
            snapshot_ancestors: Default::default(),
            block_hash_to_id: Default::default(),
            latest_id: Default::default(),
            self_ref: None,
        }));
        let self_ref = block_state_manager.clone();
        {
            let mut bm = block_state_manager.write().unwrap();
            bm.self_ref = Some(self_ref);
        }
        block_state_manager
    }

    pub fn get_value_recursively(&self, snapshot_id: SnapshotId, key: &Key) -> Option<Value> {
        let snapshot_id = self.snapshot_ancestors.get(&snapshot_id)?;
        let mut parent_snapshot = self.snapshots.get(snapshot_id);
        let cache_key = CacheKey::from(key.clone());
        while parent_snapshot.is_some() {
            let snapshot = parent_snapshot.unwrap();
            let value = snapshot.get_value(&cache_key);
            if value.is_some() {
                return value.map(|v| Value::from(v));
            }
            let parent_id = self.snapshot_ancestors.get(&snapshot.get_id())?;
            parent_snapshot = self.snapshots.get(parent_id);
        }

        None
    }

    pub fn stop(mut self) {
        self.self_ref = None
    }
}


impl<P: Persistence<Payload=CacheLog>> ForkTreeManager for BlockStateManager<P> {
    type Snapshot = FrozenSnapshot;
    type SnapshotRef = TreeManagerSnapshotQuery<P>;
    type BlockHash = BlockHash;

    fn get_from_block(&mut self, block_hash: &Self::BlockHash) -> Self::SnapshotRef {
        let prev_id = self.latest_id;
        self.latest_id += 1;
        let next_id = self.latest_id;
        println!("Getting new snapshot ref with id={} from block hash={}", next_id, block_hash);

        let new_snapshot_ref = TreeManagerSnapshotQuery {
            id: next_id,
            manager: ReadOnlyLock::new(self.self_ref.clone().unwrap().clone()),
        };

        self.snapshot_ancestors.insert(next_id, prev_id);

        new_snapshot_ref
    }

    fn add_snapshot(&mut self, prev_block_hash: &Self::BlockHash, next_block_hash: &Self::BlockHash, snapshot: Self::Snapshot) {
        println!("Adding snapshot prev_block_hash={} next_block_hash={} id={}", prev_block_hash, next_block_hash, snapshot.get_id());
        // Update chain
        self.chain_forks.entry(prev_block_hash.clone()).or_insert(Vec::new()).push(next_block_hash.clone());
        self.to_parent.insert(next_block_hash.clone(), prev_block_hash.clone());

        if let Some(parent_snapshot_id) = self.block_hash_to_id.get(prev_block_hash) {
            self.snapshot_ancestors.insert(snapshot.get_id().clone(), parent_snapshot_id.clone());
        }
        self.block_hash_to_id.insert(next_block_hash.clone(), snapshot.get_id());
        self.snapshots.insert(snapshot.get_id().clone(), snapshot);
    }

    fn finalize_snapshot(&mut self, block_hash: &Self::BlockHash) {
        println!("Finalizing block hash {}", block_hash);
        if let Some(snapshot_id) = self.block_hash_to_id.remove(block_hash) {
            let snapshot = self.snapshots.remove(&snapshot_id).expect("Tried to finalize non-existing snapshot: self.snapshots");
            assert_eq!(snapshot_id, snapshot.get_id());
            // Commit snapshot
            let payload = snapshot.into();
            let mut db = self.db.lock().unwrap();
            db.commit(payload);
        }

        // Clean up chain state
        match self.to_parent.remove(block_hash) {
            None => {
                println!("HM, committing empty ")
            }
            Some(parent_block_hash) => {
                let mut to_discard = self.chain_forks.remove(&parent_block_hash).unwrap();
                to_discard.retain(|hash| hash != block_hash);

                while !to_discard.is_empty() {
                    let next_to_discard = to_discard.pop().unwrap();
                    let next_children_to_discard = self.chain_forks.remove(&next_to_discard).unwrap_or(Default::default());
                    to_discard.extend(next_children_to_discard);

                    let snapshot_id = self.block_hash_to_id.remove(&next_to_discard).unwrap();
                    self.to_parent.remove(&next_to_discard).unwrap();
                    self.snapshots.remove(&snapshot_id).unwrap();
                }
            }
        };
    }
}