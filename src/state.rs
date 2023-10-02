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
        // TODO: What about parents? C
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
            cache: self.cache,
            witness: self.witness,
            parent: self.parent,
        }
    }
}


pub struct WorkingSet {
    db: DB,
    cache: CacheLog,
    witness: Witness,
    parent: Weak<FrozenStateCheckpoint>,
}

impl WorkingSet {
    /// Public interface. Reads local cache, then tries parents and then database, if parent was committed
    pub fn get(&mut self, key: &Key) -> Option<Value> {
        let cache_key = CacheKey::from(key.clone());

        // Read from own cache
        if let ValueExists::Yes(value) = self.cache.get_value(&cache_key) {
            return value.map(Value::from);
        }

        // Read from parent
        let value = if let Some(parent) = self.parent.upgrade() {
            parent.fetch(&key)
        } else {
            self.db.lock().unwrap().get(&key.to_string()).map(Value::from)
        };
        // AM I MISSING SOMETHING? SHOULD I DROP custom "witness" ?
        self.cache.add_read(key.clone().into(), value.clone().map(CacheValue::from)).unwrap();
        self.witness.track_operation(key, value.clone());
        value
    }

    pub fn set(&mut self, key: &Key, value: Value) {
        self.witness.track_operation(key, Some(value.clone()));
        self.cache.add_write(CacheKey::from(key.clone()), Some(CacheValue::from(value)));
    }

    pub fn commit(self) -> StateCheckpoint {
        StateCheckpoint {
            db: self.db,
            cache: self.cache,
            witness: self.witness,
            parent: self.parent,
        }
    }

    pub fn revert(self) -> StateCheckpoint {
        StateCheckpoint {
            db: self.db,
            cache: CacheLog::default(),
            witness: Witness::default(),
            parent: self.parent,
        }
    }
}


