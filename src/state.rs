use std::sync::{Arc, Mutex};
use sov_first_read_last_write_cache::cache::{CacheLog, ValueExists};
use sov_first_read_last_write_cache::CacheKey;
use crate::db::Database;
use crate::types::{Key, Value};
use crate::witness::Witness;

pub type DB = Arc<Mutex<Database>>;

pub trait StateSnapshot {
    fn on_top(&self) -> Self;

    fn commit(&self) -> CacheLog;
}


/// WorkingSet manages read/write and witness
pub struct WorkingSet {
    db: DB,
    cache: CacheLog,
    witness: Witness,
}

impl WorkingSet {
    pub fn new(db: DB) -> Self {
        Self {
            db,
            cache: CacheLog::default(),
            witness: Witness::default(),
        }
    }

    pub fn checkpoint(self) -> StateCheckpoint {
        StateCheckpoint {
            db: self.db,
            cache: self.cache,
        }
    }

    // Operations. Only get/set, don't care about delete for simplicity


    /// Get value from local cache or database. Update witness accordingly.
    pub fn get(&mut self, key: &Key) -> Option<Value> {
        let cache_key = CacheKey::from(key.clone());
        if let ValueExists::Yes(value) = self.cache.get_value(&cache_key) {
            return value.map(Value::from);
        }

        let db = self.db.lock().unwrap();

        let key_str = key.clone().to_string();
        let value = db.get(&key_str);

        if let Some(value) = value {
            let value = Value::from(value);
            self.witness.track_operation(key, Some(value.clone()));
            self.cache.add_read(cache_key, Some(value.clone().into())).unwrap();
            Some(value)
        } else {
            self.witness.track_operation(key, None);
            None
        }
    }


    pub fn set(&mut self, key: &Key, value: Value) {}
}


// StateCheckpoint only can be persisted or converted to WorkingSet.
pub struct StateCheckpoint {
    // Only used to pass handler to WorkingSet
    db: DB,
    cache: CacheLog,
}


impl StateCheckpoint {
    pub fn to_revertable(self) -> WorkingSet {
        WorkingSet {
            db: self.db,
            cache: self.cache,
            witness: Witness::default(),
        }
    }
}