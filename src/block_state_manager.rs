#![allow(dead_code)]
#![allow(unused_variables)]


use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::db::{Database, Persistence};
use crate::state::{BlockStateSnapshot, FrozenStateCheckpoint};
use crate::types::{Key, Value};


impl Persistence for Database {
    type Payload = FrozenStateCheckpoint;

    fn commit(&mut self, data: Self::Payload) {
        let writes = data.get_own_cache().take_writes();
        for (key, value) in writes {
            let key = Key::from(key).to_string();
            match value {
                Some(value) => self.set(&key, Value::from(value).to_string()),
                None => self.delete(&key),
            }
        }
    }
}

/// BlockState manager maintains chain of BlockStateSnapshots, commits them
pub struct BlockStateManager<S: BlockStateSnapshot, P: Persistence<Payload=S>> {
    db: Arc<Mutex<P>>,
    snapshots: HashMap<String, S>,
    to_parent: HashMap<String, String>,
    graph: HashMap<String, Vec<String>>,

}


impl<S: BlockStateSnapshot, P: Persistence<Payload=S>> BlockStateManager<S, P> {
    pub fn new(db: Arc<Mutex<P>>) -> Self {
        Self {
            db,
            // TODO: Add genesis,
            snapshots: Default::default(),
            to_parent: Default::default(),
            graph: Default::default(),
        }
    }

    pub fn get_snapshot_on_top_of(&self, block_hash: &str) -> Option<S::Checkpoint> {
        self.snapshots.get(block_hash).map(|s| s.on_top())
    }

    pub fn add_snapshot(&mut self, current_block_hash: &str, parent_block_hash: &str, snapshot: S) {
        self.snapshots.insert(current_block_hash.to_string(), snapshot);
        self.to_parent.insert(current_block_hash.to_string(), parent_block_hash.to_string());
        self.graph.entry(parent_block_hash.to_string()).or_default().push(current_block_hash.to_string());
    }

    pub fn finalize_block(&mut self, block_hash: &str) {
        // 1. snapshot of this block is removed from "snapshots" map
        let mut db = self.db.lock().unwrap();

        // This snapshot is committed to the database
        let snapshot = self.snapshots.remove(block_hash).unwrap();
        db.commit(snapshot);

        // All siblings are dropped.
        let parent = self.to_parent.remove(block_hash).unwrap();
        let mut to_discard = self.graph.remove(&parent).unwrap();
        to_discard.retain(|hash| hash != block_hash);

        while !to_discard.is_empty() {
            let next_to_discard = to_discard.pop().unwrap();

            let next_to_discard_children = self.graph.remove(&next_to_discard).unwrap();
            to_discard.extend(next_to_discard_children);
            self.to_parent.remove(&next_to_discard);
            self.snapshots.remove(&next_to_discard).unwrap();
        }
    }
}
