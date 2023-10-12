use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use sov_first_read_last_write_cache::cache::{CacheLog, ValueExists};
use sov_first_read_last_write_cache::{CacheKey, CacheValue};
use crate::block_state_manager::{QueryParents, Snapshot, SnapshotId, TreeQuery};
use crate::db::{Database, Storage};
use crate::types::{Key, Value};
use crate::witness::Witness;

pub type DB = Arc<Mutex<Database>>;


/// Represent CacheLayer that can be used in 2 ways:
///  - query own value
///  - be saved to database
pub struct FrozenSnapshot {
    id: SnapshotId,
    local_cache: CacheLog,
}

impl Debug for FrozenSnapshot {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "FrozenSnapshot<Id={:?}>", self.get_id())
    }
}

impl Snapshot for FrozenSnapshot {
    type Key = CacheKey;
    type Value = CacheValue;

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

    fn get_id(&self) -> SnapshotId {
        self.id
    }
}

impl From<FrozenSnapshot> for CacheLog {
    fn from(value: FrozenSnapshot) -> Self {
        value.local_cache
    }
}


impl Storage for Database {
    type Key = CacheKey;
    type Value = CacheValue;
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

    fn get(&self, key: &Self::Key) -> Option<Self::Value> {
        let key_string = key.to_string();
        self.data.get(&key_string).map(|v| CacheValue::from(Value::from(v.clone())))
    }
}

/// Note: S: Snapshot can be inside storage spec, together with SnapshotId, and SnapshotId is DaSpec::BlockHash
pub struct StateCheckpoint<P: Storage<Key=CacheKey, Value=CacheValue>, Q: QueryParents<Snapshot=FrozenSnapshot>> {
    cache: CacheLog,
    witness: Witness,
    parent: TreeQuery<P, Q>,
}


impl<P, Q> StateCheckpoint<P, Q>
    where
        P: Storage<Key=CacheKey, Value=CacheValue>,
        Q: QueryParents<Snapshot=FrozenSnapshot>,
{
    pub fn new(parent: TreeQuery<P, Q>) -> Self {
        Self {
            cache: Default::default(),
            witness: Default::default(),
            parent,
        }
    }

    pub fn to_revertable(self) -> WorkingSet<P, Q> {
        WorkingSet {
            cache: RevertableWriter::new(self.cache),
            witness: self.witness,
            parent: self.parent,
        }
    }

    pub fn freeze(mut self) -> (Witness, FrozenSnapshot) {
        let witness = std::mem::replace(&mut self.witness, Default::default());
        let snapshot = FrozenSnapshot {
            id: self.parent.get_id(),
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

pub struct WorkingSet<P: Storage<Key=CacheKey, Value=CacheValue>, Q: QueryParents<Snapshot=FrozenSnapshot>> {
    cache: RevertableWriter<CacheLog>,
    witness: Witness,
    parent: TreeQuery<P, Q>,
}

impl<P, Q> WorkingSet<P, Q>
    where
        P: Storage<Key=CacheKey, Value=CacheValue>,
        Q: QueryParents<Snapshot=FrozenSnapshot>,
{
    /// Public interface. Reads local cache, then tries parents and then database, if parent was committed
    pub fn get(&mut self, key: &Key) -> Option<Value> {
        let cache_key = CacheKey::from(key.clone());
        let value = self.cache.inner.get(key);
        if value.is_some() {
            return value;
        }

        let cache_value = self.parent.get_value_from_cache_layers(&cache_key);
        self.cache.writes.insert(cache_key, cache_value.clone());
        let value = cache_value.map(Value::from);
        self.witness.track_operation(key, value.clone());
        value
    }


    pub fn set(&mut self, key: &Key, value: Value) {
        self.witness.track_operation(key, Some(value.clone()));
        self.cache.inner.set(key, value);
    }


    pub fn commit(self) -> StateCheckpoint<P, Q> {
        StateCheckpoint {
            cache: self.cache.commit(),
            witness: self.witness,
            parent: self.parent,
        }
    }

    pub fn revert(self) -> StateCheckpoint<P, Q> {
        StateCheckpoint {
            cache: self.cache.revert(),
            witness: Witness::default(),
            parent: self.parent,
        }
    }
}
