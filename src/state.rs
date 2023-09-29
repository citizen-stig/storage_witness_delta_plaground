#![allow(dead_code)]
#![allow(unused_variables)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::db::{Database};
use crate::witness::Witness;

pub type DB = Arc<RwLock<Database>>;


pub trait State {
    /// Creates new snapshot, that can read up to all previously non committed snapshots
    fn snapshot(&self) -> Self;

    // ??
    // fn erase(&self);
    // fn commit(&self);
}


/// Requirement from rollup-interface
///  - Should make minimal restriction about implementation.
///  - Only should highlight what is expected from STF::apply_slot, basically to be stateless
///



/// Requirements from sov-modules-api / sov-state
///
/// - witness should be only tracked only for accesses outside of current snapshot
/// - snapshot should be able to correctly read data
///     from previous snapshot before resorting to the database
/// - snapshot should treat reading from previous snapshot as reading from database,
///     saving it in its own cache and updating witness
/// - whole machinery need to have type safety,
///     same as we use `WorkingSet::commit()` and `StateCheckpoint::to_revertable()`,
///     so we know when state is "clean"
///  - AppTemplate should be use this solution
///



/// Checkpoint provides read-only access to its own and nested cached data
/// It has no access to "DB" and does not witness
#[derive(Default)]
pub struct Checkpoint {
    // Probably is_committed can be replaced with emptying reads/writes.
    // But then how to distinguish "empty" checkpoint, with valid parents, from committed one?
    is_committed: AtomicBool,
    // I assume that consistency between these two are managed "somehow"
    reads: HashMap<String, u64>,
    writes: HashMap<String, u64>,
    parent: RefCell<Option<Arc<Checkpoint>>>,
}


impl Checkpoint {
    pub fn get_value(&self, key: &str) -> Option<u64> {
        if self.is_committed.load(Ordering::Acquire) {
            return None;
        }
        if let Some(value) = self.reads.get(key) {
            return Some(*value);
        }
        if let Some(value) = self.writes.get(key) {
            return Some(*value);
        }
        if let Some(parent) = &*self.parent.borrow() {
            return parent.get_value(key);
        }
        None
    }

    pub fn get_parent(&self) -> Option<Arc<Checkpoint>> {
        self.parent.borrow().clone()
    }

    pub fn commit(&self, db: DB) {
        if let Some(parent) = &*self.parent.borrow() {
            parent.commit(db.clone());
            self.parent.replace(None);
        }
        // Ideally whole block should be atomic, but should work for purpose of API design
        self.is_committed.store(true, Ordering::Release);
        let mut db = db.write().unwrap();
        for (key, value) in &self.writes {
            if *value == 0 {
                db.delete(key);
            } else {
                db.set(key, *value);
            }
        }
    }
}


impl State for Arc<Checkpoint> {
    fn snapshot(&self) -> Self {
        Arc::new(Checkpoint {
            is_committed: AtomicBool::new(false),
            reads: Default::default(),
            writes: Default::default(),
            parent: RefCell::new(Some(self.clone())),
        })
    }
}

/// Delta manages read and write operations with cache, db and witness
/// Similar to `WorkingSet` in sov-modules-api, but `WorkingSet` actually manages 2 deltas: provable and non provable.
pub struct Delta {
    db: DB,
    reads: RefCell<HashMap<String, u64>>,
    writes: HashMap<String, u64>,
    witness: Witness,
    parent_cache: Option<Arc<Checkpoint>>,
}

impl Delta {
    pub fn new(db: DB) -> Self {
        Self {
            db,
            witness: Default::default(),
            reads: Default::default(),
            writes: Default::default(),
            parent_cache: None,
        }
    }

    pub fn with_parent(db: DB, parent: Arc<Checkpoint>) -> Self {
        Self {
            db,
            witness: Default::default(),
            reads: Default::default(),
            writes: Default::default(),
            parent_cache: Some(parent),

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
            return Some(value);
        }

        if let Some(parent) = &self.parent_cache {
            let value_from_parent = parent.get_value(key);
            if let Some(value) = value_from_parent {
                self.reads.borrow_mut().insert(key.to_string(), value);
                self.witness.track_operation(key, Some(value));
                return Some(value);
            }
        }

        let value = self.db.read().unwrap().get(key);
        if let Some(v) = value {
            self.reads.borrow_mut().insert(key.to_string(), v);
        }
        self.witness.track_operation(key, value);
        value
    }


    pub fn set(&mut self, key: &str, value: u64) {
        self.witness.track_operation(key, Some(value));
        self.writes.insert(key.to_string(), value);
        if self.reads.borrow().contains_key(key) {
            self.reads.borrow_mut().remove(key);
        }
    }

    fn delete(&mut self, key: &str) {
        self.witness.track_operation(key, None);
        if self.writes.contains_key(key) {
            self.writes.remove(key);
        } else {
            // Type can be better, but it's not important for now
            self.writes.insert(key.to_string(), 0);
        }
        if self.reads.borrow().contains_key(key) {
            self.reads.borrow_mut().remove(key);
        }
    }

    pub fn freeze(self) -> (Witness, Arc<Checkpoint>) {
        let checkpoint = Arc::new(Checkpoint {
            is_committed: AtomicBool::new(false),
            reads: self.reads.into_inner(),
            writes: self.writes,
            parent: RefCell::new(self.parent_cache),
        });
        (self.witness, checkpoint)
    }
}


/// Working set, just to mimic the real one from sov-modules-api
/// Basically it holds delta, and aux_delta,
pub struct WorkingSet {
    delta: Delta,
    // To justify existence of this struct
    aux_delta: u64,
}

impl WorkingSet {
    pub fn new(storage: DB) -> Self {
        Self {
            delta: Delta::new(storage),
            aux_delta: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod checkpoint {
        use std::sync::atomic::Ordering;
        use super::*;

        #[test]
        fn default() {
            let checkpoint = Checkpoint::default();
            assert!(checkpoint.parent.borrow().is_none());
            assert!(checkpoint.writes.is_empty());
            assert!(checkpoint.reads.is_empty());
            assert!(!checkpoint.is_committed.load(Ordering::SeqCst));
        }

        #[test]
        fn get_values_non_committed() {
            let mut checkpoint_1 = Checkpoint {
                is_committed: AtomicBool::new(false),
                reads: Default::default(),
                writes: Default::default(),
                parent: RefCell::new(None),
            };

            checkpoint_1.writes.insert("a".to_string(), 1);
            checkpoint_1.reads.insert("b".to_string(), 2);


            assert_eq!(Some(1), checkpoint_1.get_value("a"));
            assert_eq!(Some(2), checkpoint_1.get_value("b"));
            assert!(checkpoint_1.get_value("c").is_none());


            let mut checkpoint_2 = Checkpoint {
                is_committed: AtomicBool::new(false),
                reads: Default::default(),
                writes: Default::default(),
                parent: RefCell::new(Some(Arc::new(checkpoint_1))),
            };

            checkpoint_2.reads.insert("c".to_string(), 3);
            checkpoint_2.writes.insert("d".to_string(), 4);


            assert_eq!(Some(1), checkpoint_2.get_value("a"));
            assert_eq!(Some(2), checkpoint_2.get_value("b"));
            assert_eq!(Some(3), checkpoint_2.get_value("c"));
            assert_eq!(Some(4), checkpoint_2.get_value("d"));
        }

        #[test]
        fn get_values_committed() {
            let mut checkpoint_1 = Checkpoint {
                is_committed: AtomicBool::new(false),
                reads: Default::default(),
                writes: Default::default(),
                parent: RefCell::new(None),
            };
            checkpoint_1.writes.insert("a".to_string(), 1);


            let checkpoint_1 = Arc::new(checkpoint_1);

            let mut checkpoint_2 = Checkpoint {
                is_committed: AtomicBool::new(false),
                reads: Default::default(),
                writes: Default::default(),
                parent: RefCell::new(Some(checkpoint_1.clone())),
            };
            checkpoint_2.writes.insert("b".to_string(), 2);


            let mut checkpoint_3 = Checkpoint {
                is_committed: AtomicBool::new(false),
                reads: Default::default(),
                writes: Default::default(),
                parent: RefCell::new(Some(Arc::new(checkpoint_2))),
            };
            checkpoint_3.writes.insert("c".to_string(), 3);

            assert_eq!(Some(1), checkpoint_3.get_value("a"));
            assert_eq!(Some(2), checkpoint_3.get_value("b"));
            assert_eq!(Some(3), checkpoint_3.get_value("c"));

            // First checkpoint got committed somehow

            checkpoint_1.is_committed.store(true, Ordering::SeqCst);

            assert!(checkpoint_3.get_value("a").is_none());
            assert_eq!(Some(2), checkpoint_3.get_value("b"));
            assert_eq!(Some(3), checkpoint_3.get_value("c"));
        }

        #[test]
        fn tree_of_checkpoints() {
            let mut checkpoint_1 = Checkpoint {
                is_committed: AtomicBool::new(false),
                reads: Default::default(),
                writes: Default::default(),
                parent: RefCell::new(None),
            };
            checkpoint_1.writes.insert("a".to_string(), 1);


            let checkpoint_1 = Arc::new(checkpoint_1);

            let mut checkpoint_2 = Checkpoint {
                is_committed: AtomicBool::new(false),
                reads: Default::default(),
                writes: Default::default(),
                parent: RefCell::new(Some(checkpoint_1.clone())),
            };

            checkpoint_2.writes.insert("b".to_string(), 2);

            let mut checkpoint_3 = Checkpoint {
                is_committed: AtomicBool::new(false),
                reads: Default::default(),
                writes: Default::default(),
                parent: RefCell::new(Some(checkpoint_1.clone())),
            };

            checkpoint_3.writes.insert("c".to_string(), 3);

            assert_eq!(Some(1), checkpoint_2.get_value("a"));
            assert_eq!(Some(1), checkpoint_3.get_value("a"));
            assert_eq!(Some(2), checkpoint_2.get_value("b"));
            assert_eq!(Some(3), checkpoint_3.get_value("c"));
            assert!(checkpoint_3.get_value("b").is_none());
            assert!(checkpoint_2.get_value("c").is_none());
        }
    }

    mod delta {
        use super::*;

        #[test]
        fn new_from_empty_database() {
            let db = Arc::new(RwLock::new(Database::default()));

            let mut delta = Delta::new(db.clone());
            assert!(delta.parent_cache.is_none());

            delta.set("a", 1);
            delta.set("b", 2);

            assert_eq!(Some(1), delta.get("a"));
            assert_eq!(Some(2), delta.get("b"));
            assert!(delta.get("c").is_none());

            delta.delete("a");
            assert_eq!(None, delta.get("a"));
            assert_eq!(Some(2), delta.get("b"));

            assert_eq!(5, delta.witness.len());
        }

        #[test]
        fn pull_from_database() {
            let db = Arc::new(RwLock::new(Database::default()));
            {
                let mut locked = db.write().unwrap();
                locked.set("a", 3);
            }
            let delta = Delta::new(db.clone());
            assert!(delta.parent_cache.is_none());
            assert_eq!(Some(3), delta.get("a"));
        }

        #[test]
        fn pull_from_parent() {
            let db = Arc::new(RwLock::new(Database::default()));

            {
                let mut locked = db.write().unwrap();
                locked.set("b", 10);
            }

            let mut delta = Delta::new(db.clone());
            assert!(delta.parent_cache.is_none());
            delta.set("a", 4);

            let (_witness, checkpoint) = delta.freeze();

            let delta_2 = Delta::with_parent(db.clone(), checkpoint.clone());

            assert_eq!(Some(4), delta_2.get("a"));
            assert_eq!(Some(10), delta_2.get("b"));
            assert!(checkpoint.get_value("b").is_none());
        }
    }
}