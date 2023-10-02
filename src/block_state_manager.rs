#![allow(dead_code)]
#![allow(unused_variables)]


/// Manages snapshots of the state corresponding to particular block
/// 1 snapshot - 1 block
/// When block is finalized, BlockStateManager, knows how to discard orphans, for example, because it tracks all snapshots by height
pub struct BlockStateManager {
    // database: DB,
    //
    // snapshots: HashMap<String, Arc<Checkpoint>>,
    // to_parent: HashMap<String, String>,
    // graph: HashMap<String, Vec<String>>,
}


impl BlockStateManager {

    // pub fn get_snapshot_on_top_of(&self, block_hash: &str) -> Option<Arc<>> {
    //     self.snapshots.get(block_hash).map(|s| s.snapshot())
    // }

    pub fn add_snapshot(&mut self, current_block_hash: &str, parent_block_hash: &str, snapshot: String) {
        // self.snapshots.insert(current_block_hash.to_string(), snapshot);
        // self.to_parent.insert(current_block_hash.to_string(), parent_block_hash.to_string());
        // self.graph.entry(parent_block_hash.to_string()).or_default().push(current_block_hash.to_string());
    }

    pub fn finalize_block(&mut self, block_hash: &str) {
    }
}