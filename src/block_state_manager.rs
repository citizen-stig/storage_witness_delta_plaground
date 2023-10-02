#![allow(dead_code)]
#![allow(unused_variables)]


use std::collections::HashMap;
use crate::db::persist_cache;
use crate::state::{StateSnapshot, DB};

/// Manages snapshots of the state corresponding to particular block
/// 1 snapshot - 1 block
/// When block is finalized, BlockStateManager, knows how to discard orphans, for example, because it tracks all snapshots by height
pub struct BlockStateManager<S: StateSnapshot> {
    database: DB,
    snapshots: HashMap<String, S>,
    to_parent: HashMap<String, String>,
    graph: HashMap<String, Vec<String>>,
}


impl<S: StateSnapshot> BlockStateManager<S> {
    pub fn get_snapshot_on_top_of(&self, block_hash: &str) -> Option<S> {
        self.snapshots.get(block_hash).map(|s| s.on_top())
    }

    pub fn add_snapshot(&mut self, current_block_hash: &str, parent_block_hash: &str, snapshot: S) {
        self.snapshots.insert(current_block_hash.to_string(), snapshot);
        self.to_parent.insert(current_block_hash.to_string(), parent_block_hash.to_string());
        self.graph.entry(parent_block_hash.to_string()).or_default().push(current_block_hash.to_string());
    }

    pub fn finalize_block(&mut self, block_hash: &str) {
        // Persist current snapshot to the database
        let snapshot = self.snapshots.remove(block_hash).unwrap();
        let cache_log = snapshot.commit();
        persist_cache(&mut self.database.lock().unwrap(), cache_log);

        // Discard all snapshots that are not on top of the finalized block
        let parent = self.to_parent.remove(block_hash).unwrap();
        let mut to_discard = self.graph.remove(&parent).unwrap();
        to_discard.retain(|hash| hash != block_hash);
        // TODO: Insert this back to parent

        // Parent was removed when it was commited
        // self.snapshots.remove(&parent).unwrap().commit();

        while !to_discard.is_empty() {
            let next_to_discard = to_discard.pop().unwrap();

            let next_to_discard_children = self.graph.remove(&next_to_discard).unwrap();
            to_discard.extend(next_to_discard_children);
            self.to_parent.remove(&next_to_discard);
            let discarded_snapshot = self.snapshots.remove(&next_to_discard).unwrap();
            discarded_snapshot.commit();
        }
    }
}