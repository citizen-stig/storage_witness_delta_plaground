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

    fn get_value(&self, key: &Self::Key) -> Option<Self::Value>;
    fn get_id(&self) -> Self::Id;
    fn on_top_of(parent: &Self, new_id: Self::Id) -> Self;
}



// This is something that managers parent/child relation between snapshots and according block hashes
pub trait StateTreeManager {
    type Snapshot: Snapshot;
    type BlockHash;

    /// Creates new snapshot on top of block_hash
    /// If block hash is not present, consider it as genesis
    fn get_from_block(&mut self, block_hash: &Self::BlockHash) -> Self::Snapshot;

    /// Adds new snapshot with given block hash to the chain
    /// Implementation is responsible for maintaining connection between block hashes
    fn add_snapshot(&mut self, parent_block_hash: &Self::BlockHash, block_hash: &Self::BlockHash, snapshot: Self::Snapshot);

    fn get_value_recursive(&self, snapshot_id: <<Self as StateTreeManager>::Snapshot as Snapshot>::Id, key: &<<Self as StateTreeManager>::Snapshot as Snapshot>::Key) -> Option<<<Self as StateTreeManager>::Snapshot as Snapshot>::Value>;

    fn finalize_snapshot(&mut self, block_hash: &Self::BlockHash);
}


type SnapshotId = u64;

// Note:
pub struct SnapshotImpl<Sm: StateTreeManager> {
    id: <Sm::Snapshot as Snapshot>::Id,
    local_cache: CacheLog,
    // TODO: Maybe ReadLockGuard???
    manager: Arc<RwLock<Sm>>,
    // Note 1: it can own manager, but it can
    // If we don't want multi threading STF execution

    // Note 2: Can it be concrete implementation of BlockStateManager?
}







impl<Sm: StateTreeManager> Snapshot for SnapshotImpl<Sm> {
    type Key = <Sm::Snapshot as Snapshot>::Key;
    type Value = <Sm::Snapshot as Snapshot>::Value;
    type Id = <Sm::Snapshot as Snapshot>::Id;

    fn get_value(&self, key: &Self::Key) -> Option<Self::Value> {
        // Check local cache

        let manager = self.manager.read().unwrap();
        manager.get_value_recursive(self.id, key)
    }

    fn get_id(&self) -> Self::Id {
        self.id
    }

    fn on_top_of(parent: &Self, new_id: Self::Id) -> Self {
        Self {
            id: new_id,
            local_cache: CacheLog::default(),
            manager: parent.manager.clone(),
        }
    }
}


impl<Sm: StateTreeManager> From<SnapshotImpl<Sm>> for CacheLog {
    fn from(value: SnapshotImpl<Sm>) -> Self {
        value.local_cache
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


type BlockHash = String;

pub struct BlockStateManager<S: Snapshot, P: Persistence> {
    db: Arc<Mutex<P>>,

    // NOTES: Naive implementation, it can be simplified!

    // Chain: prev_block -> child_blocks (forks
    chain_forks: HashMap<BlockHash, Vec<BlockHash>>,
    // Reverse: child_block -> parent
    to_parent: HashMap<BlockHash, BlockHash>,

    // Snapshots
    // snapshot_id => snapshot
    snapshots: HashMap<SnapshotId, S>,
    // All ancestors of a given snapshot kid => parent
    snapshot_ancestors: HashMap<SnapshotId, SnapshotId>,

    block_hash_to_id: HashMap<BlockHash, SnapshotId>,

    // Incremental
    latest_id: S::Id,

    genesis: Option<S>,
}


impl<S: Snapshot, P: Persistence<Payload=CacheLog>> BlockStateManager<S, P> {
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

// In reality it will also have Da trait
impl<S: Snapshot<Id=u64>, P: Persistence<Payload=CacheLog>> StateTreeManager for BlockStateManager<S, P>
    where S: Into<CacheLog>
{
    type Snapshot = S;
    type BlockHash = BlockHash;


    fn get_from_block(&mut self, block_hash: &Self::BlockHash) -> Self::Snapshot {
        let prev_id = self.latest_id;
        self.latest_id += 1;
        let next_id = self.latest_id;
        let new_snapshot = match self.block_hash_to_id.get(block_hash) {
            None => {
                let genesis = self.genesis.as_ref().unwrap();
                S::on_top_of(genesis, next_id)
            }
            Some(snapshot_id) => {
                // TODO: Assert prev_id == parent_snapshot_id.
                let parent_snapshot = self.snapshots.get(snapshot_id).expect("inconsistent snapshot mapping from parent block hash");

                S::on_top_of(parent_snapshot, next_id)
            }
        };
        self.snapshot_ancestors.insert(next_id, prev_id);

        new_snapshot
    }

    fn add_snapshot(&mut self, parent_block_hash: &Self::BlockHash, block_hash: &Self::BlockHash, snapshot: Self::Snapshot) {
        // Update chain
        self.chain_forks.entry(parent_block_hash.clone()).or_insert(Vec::new()).push(block_hash.clone());
        self.to_parent.insert(block_hash.clone(), parent_block_hash.clone());

        let parent_snapshot_id = self.block_hash_to_id.get(parent_block_hash).unwrap();
        self.snapshot_ancestors.insert(snapshot.get_id().clone(), parent_snapshot_id.clone());
        self.snapshots.insert(snapshot.get_id().clone(), snapshot);
    }

    fn get_value_recursive(&self, from_snapshot_id: <<Self as StateTreeManager>::Snapshot as Snapshot>::Id, key: &<<Self as StateTreeManager>::Snapshot as Snapshot>::Key) -> Option<<<Self as StateTreeManager>::Snapshot as Snapshot>::Value> {
        // We consider it checked its own cache before locking us.

        let mut parent_id = self.snapshot_ancestors.get(&from_snapshot_id);

        while parent_id.is_some() {
            // TODO: can be simplified
            let current_id = parent_id.unwrap();
            match self.snapshots.get(current_id) {
                None => {
                    return None;
                }
                Some(snapshot) => {
                    let value = snapshot.get_value(key);
                    if value.is_some() {
                        return value;
                    }
                }
            }
            parent_id = self.snapshot_ancestors.get(current_id);
        }

        None
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
        let payload = snapshot.into();
        let mut db = self.db.lock().unwrap();
        db.commit(payload);
    }
}

// Combining with existing sov-api



pub struct StateCheckpoint<Sm: StateTreeManager> {
    db: DB,
    cache: CacheLog,
    witness: Witness,
    parent: SnapshotImpl<Sm>,
}

impl<Sm: StateTreeManager> From<StateCheckpoint<Sm>> for SnapshotImpl<Sm> {
    fn from(value: StateCheckpoint<Sm>) -> Self {
        SnapshotImpl {
            id: value.parent.id,
            local_cache: value.parent.local_cache,
            manager: value.parent.manager,
        }
    }
}

impl<Sm: StateTreeManager> StateCheckpoint<Sm> {
    pub fn new(db: DB, parent: SnapshotImpl<Sm>) -> Self {
        Self {
            db,
            cache: Default::default(),
            witness: Default::default(),
            parent,
        }
    }

    pub fn to_revertable(self) -> WorkingSet<Sm> {
        WorkingSet {
            db: self.db,
            cache: RevertableWriter::new(self.cache),
            witness: self.witness,
            parent: self.parent,
        }
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

pub struct WorkingSet<Sm: StateTreeManager> {
    db: DB,
    cache: RevertableWriter<CacheLog>,
    witness: Witness,
    parent: SnapshotImpl<Sm>,
}


impl<Sm: StateTreeManager> WorkingSet<Sm> {
    /// Public interface. Reads local cache, then tries parents and then database, if parent was committed
    pub fn get(&mut self, key: &Key) -> Option<Value> {
        let cache_key = CacheKey::from(key.clone());

        // Read from own cache

        // Check parent recursively

        todo!()
    }

    pub fn set(&mut self, key: &Key, value: Value) {
        self.witness.track_operation(key, Some(value.clone()));
        self.cache.inner.set(key, value);
    }


    pub fn commit(self) -> StateCheckpoint<Sm> {
        StateCheckpoint {
            db: self.db,
            cache: self.cache.commit(),
            witness: self.witness,
            parent: self.parent,
        }
    }

    pub fn revert(self) -> StateCheckpoint<Sm> {
        StateCheckpoint {
            db: self.db,
            cache: self.cache.revert(),
            witness: Witness::default(),
            parent: self.parent,
        }
    }
}


// BlockStateManager: BlockHash -> Arc<FrozenStateSnapShot>.
// FrozenStateSnapShot::Weak<FrozenStateSnapshot> // parent relation
// Arc<FrozenStateSnapshot> -> StateCheckpoint.
// StateCheckpoint::Weak<FrozenStateSnapshot>
// STF::apply_slot(s: StateCheckpoint) {
//   let WorkingSet = StateCheckpoint::to_revertable(self);
//   WorkingSet::RevertableWrites<StateCheckpoint> <- actual change in cache only happens in commit,
//                                                    so writes do not pollute witness before committed
//
//   return WorkingSet::commit()
// }
// let frozen_state_checkpoint = returned_state_checkpoint.into();
// BlockStateManager.add_snapshot(Arc::new(frozen_state_checkpoint), block_hash_was_executed);

