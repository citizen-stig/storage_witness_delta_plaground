use std::sync::{Arc, Mutex};
use sov_first_read_last_write_cache::cache::{CacheLog, ValueExists};
use sov_first_read_last_write_cache::CacheKey;
use crate::db::Database;
use crate::types::{Key, Value};
use crate::witness::Witness;

pub type DB = Arc<Mutex<Database>>;

pub trait StateSnapshot {

    type ApplySlotResult;
    type StateCheckpointLike;
    /// TODO: Associated type that can hold "WorkingSet"..
    fn on_top(&self) -> Self::StateCheckpointLike;

    // /// What should it do? Several options
    // ///
    // /// 1. Return writes from this layer, so it is up to caller
    // /// 2. Recursively commit all parents up to the last committed and merge all cache logs into 1
    // /// TODO:
    // fn commit(self) -> Self::ApplySlotResult;
}

//

// a -> b -> c -> d - d'
//       \-> e -> f

//  a -> b -> c -> d
//       \-> e -> f

/// WorkingSet manages read/write and witness
pub struct WorkingSet {
    db: DB,
    cache: CacheLog,
    witness: Witness,
    parent: Arc<StateCheckpoint>,
}

impl WorkingSet {
    pub fn new(db: DB) -> Self {
        Self {
            db: db.clone(),
            cache: CacheLog::default(),
            witness: Witness::default(),
            parent: Arc::new(StateCheckpoint::new(db.clone())),
        }
    }

    fn with_parent(db: DB, parent: Arc<StateCheckpoint>) -> Self {
        Self {
            db,
            cache: Default::default(),
            witness: Default::default(),
            parent,
        }
    }

    pub fn commit(self) -> StateCheckpoint {
        StateCheckpoint {
            db: self.db,
            cache: self.cache,
            parent: Some(self.parent),
        }
    }

    pub fn revert(self) -> StateCheckpoint {
        StateCheckpoint {
            db: self.db,
            cache: Default::default(),
            parent: Some(self.parent),
        }
    }

    pub fn freeze(mut self) -> (StateCheckpoint, Witness) {
        let witness = std::mem::replace(&mut self.witness, Witness::default());
        (self.commit(), witness)
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
    parent: Option<Arc<StateCheckpoint>>,
}


impl StateCheckpoint {
    pub fn new(db: DB) -> Self {
        Self {
            db,
            cache: Default::default(),
            parent: None,
        }
    }

    pub fn to_revertable(self) -> WorkingSet {
        WorkingSet::with_parent(self.db.clone(), Arc::new(self))
    }
}


// impl StateSnapshot for StateCheckpoint {
//     fn on_top(&self) -> Self {
//        Self {
//            db: self.db.clone(),
//            cache: Default::default(),
//            // Cannot move reference into arc.
//            parent: None,
//        }
//     }
//
//     fn commit(self) -> CacheLog {
//         todo!()
//     }
// }

/// Meat

impl StateSnapshot for Arc<StateCheckpoint> {
    fn on_top(&self) -> Self {
        Arc::new(StateCheckpoint {
            db: self.db.clone(),
            cache: Default::default(),
            parent: Some(self.clone()),
        })
    }

    /// Returns cachelog
    fn commit(self) -> CacheLog {
        // Return cache log, recursively

        // Drop reference to parent


        let mut cache = CacheLog::default();
        if let Some(parent) = self.parent.clone() {
            cache.merge_left(parent.commit()).unwrap();
        }
        // NASTY: Caller should drop all references to self before calling commit
        // Maybe add can_commit? But it is runtime safety,
        let mut raw = Arc::<StateCheckpoint>::into_inner(self).unwrap();
        raw.parent = None;
        cache.merge_left(raw.cache).unwrap();
        cache
    }
}


///


/// Notes:
/// 0. Use Weak to parent
/// 1. Read from DB not in the "latest" node in chain,
/// but in the oldest not committed, by checking if Weak<Parent> is present
/// Do not use snapshoting inside STF, but just modereate...


/// We don't want STF to be able to spawn children, so weak ref will be dropped
/// So we want StateSnapshot to produce StateCheckpoint(associated type, concrete)
/// and then give it to STF. Stf returns it back, effectively, also plays with WorkingSet.

/// Questions:
///  - How to convert StateCheckpoint back to StateSnapshot?
///  - How ForkManager knows how to commit StateCheckpoint?

/// BlockStateManager<S: StateSnapshot>:: -> S::WorkingSetLike
/// Stf(S::WorkingSetLike) -> S::WorkingSetLike.
/// BlockStateManager::add_snapshot(block_hash, s: S)



/// DB needs to be generic, and this associated type,


/// Whoever constructs STF and BlockStateManager,
/// need to add constraint to convert StateCheckpoint back to StateSnapshot


