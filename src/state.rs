use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::db::{Database, DbOperations};
use crate::witness::Witness;

type DB = Arc<Database>;

/// Checkpoint provides read-only access to its own and nested cached data
/// It has no access to "DB" and does not witness
#[derive(Default)]
struct Checkpoint {
    reads: HashMap<String, u64>,
    writes: HashMap<String, u64>,
    parent: Option<Arc<Checkpoint>>,
}

impl Checkpoint {
    fn get_value(&self, key: &str) -> Option<u64> {
        if let Some(value) = self.reads.get(key) {
            return Some(*value);
        }
        if let Some(value) = self.writes.get(key) {
            return Some(*value);
        }
        if let Some(parent) = &self.parent {
            return parent.get_value(key);
        }
        None
    }
}

impl From<Checkpoint> for DbOperations {
    fn from(value: Checkpoint) -> Self {
        let mut writes = DbOperations::new();
        if let Some(parent) = value.parent {
            writes.extend(parent.into());
        }
        writes.extend(value.writes);
        writes
    }
}


/// Delta manages read and write operations with cache, db and witness
struct Delta {
    db: DB,
    reads: RefCell<HashMap<String, u64>>,
    writes: HashMap<String, u64>,
    witness: Witness,
    parent: Option<Arc<Checkpoint>>,
}

impl Delta {
    fn new(db: DB) -> Self {
        Self {
            db,
            witness: Default::default(),
            reads: Default::default(),
            writes: Default::default(),
            parent: None,
        }
    }

    fn with_parent(db: DB, parent: Arc<Checkpoint>) -> Self {
        Self {
            db,
            witness: Default::default(),
            reads: Default::default(),
            writes: Default::default(),
            parent: Some(parent),

        }
    }

    fn check_in_local_cache(&self, key: &str) -> Option<u64> {
        if let Some(value) = self.reads.borrow().get(key) {
            return Some(*value);
        }
        if let Some(value) = self.writes.get(key) {
            return Some(*value);
        }
        None
    }


    pub fn get(&self, key: &str) -> Option<u64> {
        if let Some(value) = self.check_in_local_cache(key) {
            println!("Found in local cache: {} {} ", key, value);
            return Some(value);
        }
        let value = if let Some(parent) = &self.parent {
            parent.get_value(key)
        } else {
            self.db.get(key)
        };
        if let Some(v) = value {
            self.reads.borrow_mut().insert(key.to_string(), v);
        }
        self.witness.track_operation(key, value);
        None
    }


    fn set(&mut self, key: &str, value: u64) {
        self.witness.track_operation(key, Some(value));
        self.writes.insert(key.to_string(), value);
    }

    fn delete(&mut self, key: &str) {
        self.witness.track_operation(key, None);
        if self.writes.contains_key(key) {
            self.writes.remove(key);
        } else {
            // Type can be better, but it's not important for now
            self.writes.insert(key.to_string(), 0);
        }
    }

    fn freeze(&self) -> Checkpoint {
        Checkpoint {
            is_committed: Default::default(),
            reads: self.reads.borrow().clone(),
            writes: self.writes.clone(),
            parent: self.parent.clone(),
        }
    }
}


/// Working set, just to mimic the real one from sov-modules-api
/// Basically it holds delta, and aux_delta,
struct WorkingSet {
    delta: Delta,
    // To justify existence of this struct
    aux_delta: u64,
}

impl WorkingSet {
    fn new(storage: DB) -> Self {
        Self {
            delta: Delta::new(storage),
            aux_delta: 0,
        }
    }

    fn snapshot(&self) -> Checkpoint {
        let db = self.delta.db.clone();

        Self {
            delta: Delta::with_parent(db, Arc::new(self.delta.freeze())),
            aux_delta: self.aux_delta + 1,
        }
    }
