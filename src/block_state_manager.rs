use std::collections::HashMap;
use std::sync::Arc;
use crate::state::{Checkpoint, DB, State};


/// Manages snapshots of the state corresponding to particular block
/// 1 snapshot - 1 block
/// When block is finalized, BlockStateManager, knows how to discard orphans, for example, because it tracks all snapshots by height
pub struct BlockStateManager {
    database: DB,

    snapshots: HashMap<String, Arc<Checkpoint>>,
    to_parent: HashMap<String, String>,
    graph: HashMap<String, Vec<String>>
}


impl BlockStateManager {
    pub fn new(database: DB) -> Self {
        Self {
            database,
            snapshots: Default::default(),
            to_parent: Default::default(),
            graph: Default::default(),
        }
    }

    pub fn get_snapshot_on_top_of(&self, block_hash: &str) -> Option<Arc<Checkpoint>> {
        self.snapshots.get(block_hash).map(|s| s.snapshot())
    }

    pub fn add_snapshot(&mut self, current_block_hash: &str, parent_block_hash: &str, snapshot: Arc<Checkpoint>) {
        self.snapshots.insert(current_block_hash.to_string(), snapshot);
        self.to_parent.insert(current_block_hash.to_string(), parent_block_hash.to_string());
        self.graph.entry(parent_block_hash.to_string()).or_default().push(current_block_hash.to_string());
    }

    pub fn finalize_block(&mut self, block_hash: &str) {
        let checkpoint = self.snapshots.get(block_hash).unwrap();
        checkpoint.commit(self.database.clone());

        // Eliminate children

        let parent_hash = self.to_parent.get(block_hash).unwrap();

        // Fix this
        let mut block_hashes_to_eliminate: Vec<String> = self.graph.remove(block_hash)
            .unwrap()
            .into_iter().filter(|child_block_hash| child_block_hash != block_hash)
            .collect();

        while block_hashes_to_eliminate.is_empty() {
            let next_to_eliminate = block_hashes_to_eliminate.pop().unwrap();
            let its_children = self.graph.remove(&next_to_eliminate).unwrap();
            block_hashes_to_eliminate.extend(its_children);
            self.to_parent.remove(&next_to_eliminate);
            self.snapshots.remove(&next_to_eliminate);
        }
    }
}



#[cfg(test)]
mod tests {
    use crate::block_state_manager::BlockStateManager;
    use crate::state::{Checkpoint, DB, Delta};

    #[test]
    fn new() {
        let database = DB::default();
        let block_state_manager: BlockStateManager = BlockStateManager::new(database.clone());

        assert!(block_state_manager.get_snapshot_on_top_of("genesis").is_none());
    }

    #[test]
    fn direct_chain_commit_one_by_one() {
        let database = DB::default();
        let mut block_state_manager: BlockStateManager = BlockStateManager::new(database.clone());

        // let base_checkpoint = Arc::new(Checkpoint::default());
        //
        // block_state_manager.save_snapshot("genesis", base_checkpoint);
        //
        //
        let checkpoint_block_a = {
            let checkpoint = block_state_manager.get_snapshot_on_top_of("genesis").unwrap();
            let mut working_set = Delta::with_parent(database.clone(), checkpoint);
            working_set.set("x", 1);
            let (_witness, checkpoint) = working_set.freeze();
            checkpoint
        };
        block_state_manager.save_snapshot("a", checkpoint_block_a);

        let checkpoint_block_b = {
            let checkpoint = block_state_manager.get_snapshot_on_top_of("a").unwrap();
            let mut delta = Delta::with_parent(database.clone(), checkpoint);
            delta.set("y", 2);
            let x = delta.get("x");
            assert_eq!(Some(1), x);
            let (_witness, checkpoint) = delta.freeze();
            checkpoint
        };

        block_state_manager.save_snapshot("b", checkpoint_block_b);

        let checkpoint_block_c = {
            let checkpoint = block_state_manager.get_snapshot_on_top_of("a").unwrap();
            let mut delta = Delta::with_parent(database.clone(), checkpoint);
            delta.set("z", 3);
            let x = delta.get("x");
            assert_eq!(Some(1), x);
            let y = delta.get("y");
            assert_eq!(Some(2), y);
            let (_witness, checkpoint) = delta.freeze();
            checkpoint
        };
    }
}