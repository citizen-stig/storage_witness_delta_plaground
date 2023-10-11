use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use sov_first_read_last_write_cache::cache::{CacheLog, ValueExists};
use sov_first_read_last_write_cache::{CacheKey, CacheValue};
use crate::block_state_manager::TreeQuery;
use crate::db::{Database, Persistence};
use crate::rollup_interface::Snapshot;
use crate::types::{Key, Value};
use crate::witness::Witness;

pub type DB = Arc<Mutex<Database>>;
pub type SnapshotId = u64;


///
pub struct FrozenSnapshot<I> {
    id: I,
    local_cache: CacheLog,
}

impl<I: Debug + Clone> Debug for FrozenSnapshot<I> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "FrozenSnapshot<Id={:?}>", self.get_id())
    }
}

impl<I: Clone> Snapshot for FrozenSnapshot<I> {
    type Id = I;
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

    fn get_id(&self) -> &Self::Id {
        &self.id
    }
}

impl<I> From<FrozenSnapshot<I>> for CacheLog {
    fn from(value: FrozenSnapshot<I>) -> Self {
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

/// Note: S: Snapshot can be inside spec, with Bh, and Bh assigned from DA spec from above
pub struct StateCheckpoint<P: Persistence, S: Snapshot<Id=Bh>, Bh> {
    db: DB,
    cache: CacheLog,
    witness: Witness,
    parent: TreeQuery<P, S, Bh>,
}


impl<P, S, Bh> StateCheckpoint<P, S, Bh>
    where
        P: Persistence,
        S: Snapshot<Id=Bh, Key=CacheKey, Value=CacheValue>,
        Bh: Eq + Hash + Clone
{
    pub fn new(db: DB, parent: TreeQuery<P, S, Bh>) -> Self {
        Self {
            db,
            cache: Default::default(),
            witness: Default::default(),
            parent,
        }
    }

    pub fn to_revertable(self) -> WorkingSet<P, S, Bh> {
        WorkingSet {
            db: self.db,
            cache: RevertableWriter::new(self.cache),
            witness: self.witness,
            parent: self.parent,
        }
    }

    pub fn freeze(mut self) -> (Witness, FrozenSnapshot<S::Id>) {
        let witness = std::mem::replace(&mut self.witness, Default::default());
        let snapshot = FrozenSnapshot {
            id: self.parent.get_id().clone(),
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

pub struct WorkingSet<P: Persistence, S: Snapshot<Id=Bh>, Bh> {
    db: DB,
    cache: RevertableWriter<CacheLog>,
    witness: Witness,
    parent: TreeQuery<P, S, Bh>,
}

impl<P, S, Bh> WorkingSet<P, S, Bh>
    where
        P: Persistence,
        S: Snapshot<Id=Bh, Key=CacheKey, Value=CacheValue>,
        Bh: Eq + Hash + Clone,
{
    /// Public interface. Reads local cache, then tries parents and then database, if parent was committed
    pub fn get(&mut self, key: &Key) -> Option<Value> {
        let cache_key = CacheKey::from(key.clone());
// Read from own cache
        let value = self.cache.inner.get(key);
        if value.is_some() {
            return value;
        }

// Check parent recursively
        let cache_value = match self.parent.get_value(&cache_key) {
            Some(value) => Some(value),
            None => {
                let db = self.db.lock().unwrap();
                let db_key = key.to_string();
// TODO: Ugly
                db.get(&db_key).map(|v| CacheValue::from(Value::from(v)))
            }
        };

        let cache_value = cache_value.clone();
        self.cache.writes.insert(cache_key, cache_value.clone());
        let value = cache_value.map(Value::from);
        self.witness.track_operation(key, value.clone());
        value
    }


    pub fn set(&mut self, key: &Key, value: Value) {
        self.witness.track_operation(key, Some(value.clone()));
        self.cache.inner.set(key, value);
    }


    pub fn commit(self) -> StateCheckpoint<P, S, Bh> {
        StateCheckpoint {
            db: self.db,
            cache: self.cache.commit(),
            witness: self.witness,
            parent: self.parent,
        }
    }

    pub fn revert(self) -> StateCheckpoint<P, S, Bh> {
        StateCheckpoint {
            db: self.db,
            cache: self.cache.revert(),
            witness: Witness::default(),
            parent: self.parent,
        }
    }
}
