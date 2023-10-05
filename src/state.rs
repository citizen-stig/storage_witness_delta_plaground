use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use sov_first_read_last_write_cache::cache::{CacheLog, ValueExists};
use sov_first_read_last_write_cache::{CacheKey, CacheValue};
use crate::db::{Database, Persistence};
use crate::types::{Key, Value};
use crate::witness::Witness;

pub type DB = Arc<Mutex<Database>>;


// This is something that goes into STF, STF can build StateCheckpoint out of it.
pub trait Snapshot {
    type Key;
    type Value: Clone;
    type Id: Default + Copy;

    /// Get own value, value from its own cache
    fn get_value(&self, key: &Self::Key) -> Option<Self::Value>;

    /// Helper method for mapping
    fn get_id(&self) -> Self::Id;
}


// This is something that managers parent/child relation between snapshots and according block hashes
pub trait ForkTreeManager {
    type Snapshot: Snapshot;
    type SnapshotRef;
    type BlockHash;

    // These 3 methods external to STF

    /// Creates new snapshot on top of block_hash
    /// If block hash is not present, consider it as genesis
    fn get_from_block(&mut self, block_hash: &Self::BlockHash) -> Self::SnapshotRef;

    /// Adds new snapshot with given block hash to the chain
    /// Implementation is responsible for maintaining connection between block hashes
    /// NOTE: Maybe we don't need parent, and find parent hash from id
    fn add_snapshot(&mut self, parent_block_hash: &Self::BlockHash, block_hash: &Self::BlockHash, snapshot: Self::Snapshot);

    /// Save it
    fn finalize_snapshot(&mut self, block_hash: &Self::BlockHash);
}


pub trait CacheLayerReader {
    type Snapshot: Snapshot;

    fn get_value_recursive(&self,
                           snapshot_id: <<Self as CacheLayerReader>::Snapshot as Snapshot>::Id,
                           key: &<<Self as CacheLayerReader>::Snapshot as Snapshot>::Key,
    ) -> Option<<<Self as CacheLayerReader>::Snapshot as Snapshot>::Value>;
}

type SnapshotId = u64;


pub struct FrozenSnapshot {
    id: SnapshotId,
    local_cache: CacheLog,
}

impl Snapshot for FrozenSnapshot {
    type Key = CacheKey;
    type Value = CacheValue;
    type Id = SnapshotId;

    fn get_value(&self, key: &Self::Key) -> Option<Self::Value> {
        match self.local_cache.get_value(key) {
            ValueExists::Yes(value) => {
                value
            }
            ValueExists::No => {
                None
            }
        }
    }

    fn get_id(&self) -> Self::Id {
        self.id
    }
}


impl Persistence for Database {
    type Payload = CacheLog;

    fn commit(&mut self, data: Self::Payload) {
        let writes = data.take_writes();
        for (key, value) in writes {
            let key = Key::from(key).to_string();
            match value {
                Some(value) => self.set(&key, Value::from(value).to_string()),
                None => self.delete(&key),
            }
        }
    }
}

pub struct SnapshotRefImpl<P: Persistence<Payload=CacheLog>> {
    id: SnapshotId,
    manager: Arc<RwLock<BlockStateManager<P>>>,
}

type BlockHash = String;

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

    genesis: Option<SnapshotRefImpl<P>>,
}

impl<P: Persistence<Payload=CacheLog>> BlockStateManager<P> {
    pub fn new(db: Arc<Mutex<P>>) -> Self {
        Self {
            db,
            chain_forks: Default::default(),
            to_parent: Default::default(),
            snapshots: Default::default(),
            snapshot_ancestors: Default::default(),
            block_hash_to_id: Default::default(),
            latest_id: Default::default(),
            genesis: None,
        }
    }
}


impl<P: Persistence<Payload=CacheLog>> ForkTreeManager for BlockStateManager<P> {
    type Snapshot = FrozenSnapshot;
    type SnapshotRef = SnapshotRefImpl<P>;
    type BlockHash = BlockHash;

    fn get_from_block(&mut self, block_hash: &Self::BlockHash) -> Self::SnapshotRef {
        let prev_id = self.latest_id;
        self.latest_id += 1;
        let next_id = self.latest_id;

        let genesis = self.genesis.as_ref().unwrap();

        let new_snapshot_ref = SnapshotRefImpl {
            id: next_id,
            manager: genesis.manager.clone(),
        };

        self.snapshot_ancestors.insert(next_id, prev_id);

        new_snapshot_ref
    }

    fn add_snapshot(&mut self, parent_block_hash: &Self::BlockHash, block_hash: &Self::BlockHash, snapshot: Self::Snapshot) {
        // Update chain
        self.chain_forks.entry(parent_block_hash.clone()).or_insert(Vec::new()).push(block_hash.clone());
        self.to_parent.insert(block_hash.clone(), parent_block_hash.clone());

        let parent_snapshot_id = self.block_hash_to_id.get(parent_block_hash).unwrap();
        self.snapshot_ancestors.insert(snapshot.get_id().clone(), parent_snapshot_id.clone());
        self.snapshots.insert(snapshot.get_id().clone(), snapshot);
    }

    fn finalize_snapshot(&mut self, block_hash: &Self::BlockHash) {
        let snapshot_id = self.block_hash_to_id.remove(block_hash).expect("Tried to finalize non-existing snapshot: self.block_hash_to_id");
        let snapshot = self.snapshots.remove(&snapshot_id).expect("Tried to finalize non-existing snapshot: self.snapshots");
        assert_eq!(snapshot_id, snapshot.get_id());


        // Clean up chain state
        match self.to_parent.remove(block_hash) {
            None => {
                println!("HM, committing what???")
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

        // Commit snapshot
        // let payload = snapshot.into();
        // let mut db = self.db.lock().unwrap();
        // db.commit(payload);
    }
}


// Combining with existing sov-api
pub struct StateCheckpoint<P: Persistence<Payload=CacheLog>> {
    db: DB,
    cache: CacheLog,
    witness: Witness,
    parent: SnapshotRefImpl<P>,
}


impl<P: Persistence<Payload=CacheLog>> StateCheckpoint<P> {
    fn new(db: DB, parent: SnapshotRefImpl<P>) -> Self {
        Self {
            db,
            cache: Default::default(),
            witness: Default::default(),
            parent,
        }
    }

    pub fn to_revertable(self) -> WorkingSet<P> {
        WorkingSet {
            db: self.db,
            cache: RevertableWriter::new(self.cache),
            witness: self.witness,
            parent: self.parent,
        }
    }

    fn freeze(mut self) -> (Witness, FrozenSnapshot) {
        let witness = std::mem::replace(&mut self.witness, Default::default());
        let snapshot = FrozenSnapshot {
            id: self.parent.id,
            local_cache: self.cache,
        };

        (witness, snapshot)
    }
}

pub trait StateReaderAndWriter {
    fn get(&mut self, key: &Key) -> Option<Value>;
    fn set(&mut self, key: &Key, value: Value);
    fn delete(&mut self, key: &Key);
}

impl StateReaderAndWriter for CacheLog {
    fn get(&mut self, key: &Key) -> Option<Value> {
        let cache_key = CacheKey::from(key.clone());
        match self.get_value(&cache_key) {
            ValueExists::Yes(some) => {
                some.map(|v| Value::from(v))
            }
            ValueExists::No => {
                None
            }
        }
    }

    fn set(&mut self, key: &Key, value: Value) {
        let cache_key = CacheKey::from(key.clone());
        let value = CacheValue::from(value);
        self.add_write(cache_key, Some(value));
    }

    fn delete(&mut self, key: &Key) {
        let cache_key = CacheKey::from(key.clone());
        self.add_write(cache_key, None);
    }
}


struct RevertableWriter<T> {
    inner: T,
    writes: HashMap<CacheKey, Option<CacheValue>>,
}


impl<T> RevertableWriter<T>
    where
        T: StateReaderAndWriter,
{
    fn new(inner: T) -> Self {
        Self {
            inner,
            writes: Default::default(),
        }
    }

    fn commit(mut self) -> T {
        for (k, v) in self.writes.into_iter() {
            if let Some(v) = v {
                self.inner.set(&k.into(), v.into());
            } else {
                self.inner.delete(&k.into());
            }
        }

        self.inner
    }

    fn revert(self) -> T {
        self.inner
    }
}

pub struct WorkingSet<P: Persistence<Payload=CacheLog>> {
    db: DB,
    cache: RevertableWriter<CacheLog>,
    witness: Witness,
    parent: SnapshotRefImpl<P>,
}

impl<P: Persistence<Payload=CacheLog>> WorkingSet<P> {
    /// Public interface. Reads local cache, then tries parents and then database, if parent was committed
    pub fn get(&mut self, key: &Key) -> Option<Value> {
        let cache_key = CacheKey::from(key.clone());
        // Read from own cache
        let value = self.cache.inner.get(key);
        if value.is_some() {
            return value;
        }


        // Check parent recursively
        // let _value_from_parent = self.parent.get_value_from_parent(key);

        // Reading from database
        // Add handling for add_read and putting it in local cache

        todo!()
    }

    pub fn set(&mut self, key: &Key, value: Value) {
        self.witness.track_operation(key, Some(value.clone()));
        self.cache.inner.set(key, value);
    }


    pub fn commit(self) -> StateCheckpoint<P> {
        StateCheckpoint {
            db: self.db,
            cache: self.cache.commit(),
            witness: self.witness,
            parent: self.parent,
        }
    }

    pub fn revert(self) -> StateCheckpoint<P> {
        StateCheckpoint {
            db: self.db,
            cache: self.cache.revert(),
            witness: Witness::default(),
            parent: self.parent,
        }
    }
}
