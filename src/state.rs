use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use sov_first_read_last_write_cache::cache::{CacheLog, ValueExists};
use sov_first_read_last_write_cache::{CacheKey, CacheValue};
use crate::db::Database;
use crate::types::{Key, Value};
use crate::witness::Witness;

pub type DB = Arc<Mutex<Database>>;

pub trait BlockStateSnapshot {
    /// Type that can be consumed by STF
    type Checkpoint; // StateCheckpoint

    fn on_top(&self) -> Self::Checkpoint;
}


pub struct FrozenStateCheckpoint {
    db: DB,
    cache: CacheLog,
    parent: Weak<FrozenStateCheckpoint>,
}

pub struct StateCheckpoint {
    db: DB,
    cache: CacheLog,
    witness: Witness,
    parent: Weak<FrozenStateCheckpoint>,
}


impl FrozenStateCheckpoint {
    fn fetch(&self, key: &Key) -> Option<Value> {
        let cache_key = CacheKey::from(key.clone());

        // Read from own cache
        if let ValueExists::Yes(value) = self.cache.get_value(&cache_key) {
            return value.map(Value::from);
        }

        // Read from parent
        if let Some(parent) = self.parent.upgrade() {
            parent.fetch(key)
        } else {
            self.db.lock().unwrap().get(&key.to_string()).map(Value::from)
        }
    }

    pub fn get_own_cache(self) -> CacheLog {
        self.cache
    }
}

impl BlockStateSnapshot for Arc<FrozenStateCheckpoint> {
    type Checkpoint = StateCheckpoint;

    fn on_top(&self) -> Self::Checkpoint {
        StateCheckpoint {
            db: self.db.clone(),
            cache: Default::default(),
            witness: Default::default(),
            parent: Arc::<FrozenStateCheckpoint>::downgrade(&self),
        }
    }
}

impl From<StateCheckpoint> for Arc<FrozenStateCheckpoint> {
    fn from(value: StateCheckpoint) -> Self {
        let frozen = FrozenStateCheckpoint {
            db: value.db,
            cache: value.cache,
            parent: value.parent,
        };
        Arc::new(frozen)
    }
}

impl StateCheckpoint {
    pub fn to_revertable(self) -> WorkingSet {
        WorkingSet {
            db: self.db,
            cache: RevertableWriter::new(self.cache),
            witness: self.witness,
            parent: self.parent,
        }
    }
}

pub(crate) trait StateReaderAndWriter {
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


pub struct WorkingSet {
    db: DB,
    /// TODO: Add RevertableWriter<CacheLog>
    cache: RevertableWriter<CacheLog>,
    witness: Witness,
    parent: Weak<FrozenStateCheckpoint>,
}

impl WorkingSet {
    /// Public interface. Reads local cache, then tries parents and then database, if parent was committed
    pub fn get(&mut self, key: &Key) -> Option<Value> {
        let cache_key = CacheKey::from(key.clone());


        // Read from own cache
        if let ValueExists::Yes(value) = self.cache.inner.get_value(&cache_key) {
            return value.map(Value::from);
        }

        // Read from parent
        let value = if let Some(parent) = self.parent.upgrade() {
            parent.fetch(&key)
        } else {
            self.db.lock().unwrap().get(&key.to_string()).map(Value::from)
        };
        // AM I MISSING SOMETHING? SHOULD I DROP custom "witness" ?
        self.cache.inner.add_read(key.clone().into(), value.clone().map(CacheValue::from)).unwrap();
        self.witness.track_operation(key, value.clone());
        value
    }

    pub fn set(&mut self, key: &Key, value: Value) {
        self.witness.track_operation(key, Some(value.clone()));
        self.cache.inner.set(key, value);
    }


    pub fn commit(self) -> StateCheckpoint {
        StateCheckpoint {
            db: self.db,
            cache: self.cache.commit(),
            witness: self.witness,
            parent: self.parent,
        }
    }

    pub fn revert(self) -> StateCheckpoint {
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

