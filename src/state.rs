use std::collections::HashMap;
use std::fmt::Formatter;
use std::sync::{Arc, Mutex};
use sov_first_read_last_write_cache::cache::{CacheLog, ValueExists};
use sov_first_read_last_write_cache::{CacheKey, CacheValue};
use crate::block_state_manager::BlockStateManager;
use crate::db::{Database, Persistence};
use crate::rollup_interface::Snapshot;
use crate::types::{Key, ReadOnlyLock, Value};
use crate::witness::Witness;

pub type DB = Arc<Mutex<Database>>;


// Potential trait for further generalization
// pub trait CacheLayerReader {
//     type Snapshot: Snapshot;
//
//     fn get_value_recursive(&self,
//                            snapshot_id: <<Self as CacheLayerReader>::Snapshot as Snapshot>::Id,
//                            key: &<<Self as CacheLayerReader>::Snapshot as Snapshot>::Key,
//     ) -> Option<<<Self as CacheLayerReader>::Snapshot as Snapshot>::Value>;
// }

pub type SnapshotId = u64;


pub struct FrozenSnapshot {
    id: SnapshotId,
    local_cache: CacheLog,
}

impl std::fmt::Debug for FrozenSnapshot {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "FrozenSnapshot<Id={:?}>", self.id)
    }
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

impl From<FrozenSnapshot> for CacheLog {
    fn from(value: FrozenSnapshot) -> Self {
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

pub struct TreeManagerSnapshotQuery<P: Persistence<Payload=CacheLog>> {
    pub id: SnapshotId,
    pub manager: ReadOnlyLock<BlockStateManager<P>>,
}


impl<P: Persistence<Payload=CacheLog>> TreeManagerSnapshotQuery<P> {
    pub fn new(id: SnapshotId, manager: ReadOnlyLock<BlockStateManager<P>>) -> Self {
        Self {
            id,
            manager,
        }
    }

    fn get_value(&self, key: &Key) -> Option<Value> {
        let manager = self.manager.read().unwrap();
        manager.get_value_recursively(self.id, key)
    }
}


// Combining with existing sov-api
pub struct StateCheckpoint<P: Persistence<Payload=CacheLog>> {
    db: DB,
    cache: CacheLog,
    witness: Witness,
    parent: TreeManagerSnapshotQuery<P>,
}


impl<P: Persistence<Payload=CacheLog>> StateCheckpoint<P> {
    pub fn new(db: DB, parent: TreeManagerSnapshotQuery<P>) -> Self {
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

    pub fn freeze(mut self) -> (Witness, FrozenSnapshot) {
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
    parent: TreeManagerSnapshotQuery<P>,
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
        let value = match self.parent.get_value(key) {
            Some(value) => Some(value),
            None => {
                let db = self.db.lock().unwrap();
                let db_key = key.to_string();
                db.get(&db_key).map(|v| Value::from(v))
            }
        };

        let cache_value = value.clone().map(|v| CacheValue::from(v.clone()));
        self.cache.writes.insert(cache_key, cache_value);
        self.witness.track_operation(key, value.clone());

        value
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
