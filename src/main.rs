#![allow(dead_code)]
#![allow(unused_variables)]

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, RwLock};
use crate::db::{Database};
use crate::state::{BlockStateManager, ForkTreeManager, SnapshotRefImpl};
use crate::stf::{Operation, SampleSTF, STF};

mod db;
mod witness;
mod state;
mod block_state_manager;
mod stf;
mod types;

fn runner<Stf, Fm, B, Bh>(
    mut stf: Stf,
    fork_manager: Arc<RwLock<Fm>>,
    // Simulates arrival of DA blocks
    chain: Vec<Bh>,
    // Simulates what forks are created at given parent block
    mut batches: HashMap<Bh, Vec<(Bh, Vec<B>)>>)
    where
    // This constraint is for a map.
        Bh: Eq + Hash,
        Stf: STF<BlobTransaction=B>,
        Fm: ForkTreeManager<
            SnapshotRef=<Stf as STF>::CheckpointRef, Snapshot=<Stf as STF>::Snapshot,
            BlockHash=Bh
        >,
// Note: How to put bound on INTO
{
    for current_block_hash in chain {
        let forks = batches.remove(&current_block_hash).unwrap();
        for (child_block_hash, blob) in forks {
            let snapshot_ref = {
                let mut fm = fork_manager.write().unwrap();
                fm.get_from_block(&current_block_hash)
            };
            let (_state_root, _witness, snapshot) = stf.apply_slot(snapshot_ref, blob);
            {
                let mut fm = fork_manager.write().unwrap();
                fm.add_snapshot(&current_block_hash, &child_block_hash, snapshot);
            }
        }
    }
}


fn main() {
    let db = Arc::new(Mutex::new(Database::default()));
    let stf: SampleSTF<Database> = SampleSTF::new(db.clone());

    // Bootstrap fork_state_manager
    let fork_state_manager = Arc::new(RwLock::new(BlockStateManager::new(db.clone())));
    let dummy_snapshot_ref = SnapshotRefImpl::<Database>::new(0, fork_state_manager.clone());
    {
        let mut fm = fork_state_manager.write().unwrap();
        fm.set_genesis(dummy_snapshot_ref);
    }

    // Bootstrap operations
    let operations: Vec<Operation> = vec![];
}
