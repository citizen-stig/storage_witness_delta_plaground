use std::collections::HashMap;
use crate::state::State;


/// Manages snapshots of the state corresponding to particular block
pub struct BlockStateManager<D, S: State> {
    database: D,
    // Block hash -> state snapshot
    chain: HashMap<String, S>,
}

impl<D, S: State> BlockStateManager<D, S> {
    pub fn new(database: D) -> Self {
        Self {
            database,
            chain: Default::default(),
        }
    }

    pub fn get_snapshot_on_top_of(&self, block_hash: &str) -> Option<S> {
        self.chain.get(block_hash).map(|s| s.snapshot())
    }

    pub fn finalize_block(&mut self, block_hash: &str) {
        // let snapshot = self.database.snapshot();
        // self.chain.insert(block_hash.to_string(), snapshot);
    }

    pub fn save_snapshot(&mut self, block_hash: &str, snapshot: S) {
        // We ignore overwrite for now
        self.chain.insert(block_hash.to_string(), snapshot);
    }
}

#[cfg(test)]
mod tests {
}