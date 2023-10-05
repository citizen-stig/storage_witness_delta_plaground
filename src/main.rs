#![allow(dead_code)]
#![allow(unused_variables)]

use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::sync::{Arc, Mutex, RwLock};
use crate::db::{Database};
use crate::state::{BlockStateManager, ForkTreeManager, SnapshotRefImpl};
use crate::stf::{Operation, SampleSTF, STF};
use crate::types::{Key, Value};

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
    // Matches length of chain, and when value is present this block is
    finalized_blocks: Vec<Option<Bh>>,
    // Simulates what forks are created at given parent block
    mut batches: HashMap<Bh, Vec<(Bh, Vec<B>)>>)
    where
    // This constraint is for a map.
        Bh: Eq + Hash + Display,
        Stf: STF<BlobTransaction=B>,
        Fm: ForkTreeManager<
            SnapshotRef=<Stf as STF>::CheckpointRef, Snapshot=<Stf as STF>::Snapshot,
            BlockHash=Bh
        >,
// Note: How to put bound on INTO
{
    assert_eq!(chain.len(), finalized_blocks.len());
    for (current_block_hash, finalized_block_hash) in chain.into_iter().zip(finalized_blocks.into_iter()) {
        println!("== Iterating over block {}", current_block_hash);
        let forks = batches.remove(&current_block_hash).unwrap_or_default();
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
        if let Some(finalized_block_hash) = finalized_block_hash {
            let mut fm = fork_manager.write().unwrap();
            fm.finalize_snapshot(&finalized_block_hash);
        }
        println!("== ========");
    }
}

macro_rules! hashmap {
    ($( $key:expr => $val:expr ),*) => {{
        let mut map = std::collections::HashMap::new();
        $( map.insert($key, $val); )*
        map
    }};
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

    // Desired Chain:
    //       /-> g
    // a -> b -> c -> d -> e
    //  \-> e -> f -> h
    //       \-> k

    // Current chain
    // *    *    *
    // a -> b -> c -> d
    //

    let block_hash_a = "a".to_string();
    let block_hash_b = "b".to_string();
    let block_hash_c = "c".to_string();
    let block_hash_d = "d".to_string();
    let chain: Vec<String> = vec![block_hash_a.clone(), block_hash_b.clone(), block_hash_c.clone(), block_hash_d.clone()];

    let finalized: Vec<Option<String>> =
        vec![
            None,
            Some(block_hash_a.clone()),
            Some(block_hash_b.clone()),
            Some(block_hash_c.clone()),
        ];


    let batch_1 = vec![
        Operation::Get(Key::from("x".to_string())),
        Operation::Set(Key::from("x".to_string()), Value::from("1".to_string())),
    ];
    let batch_2 = vec![
        Operation::Get(Key::from("x".to_string())),
        Operation::Set(Key::from("y".to_string()), Value::from("2".to_string())),
    ];
    let batch_3 = vec![
        Operation::Set(Key::from("x".to_string()), Value::from("3".to_string())),
        Operation::Get(Key::from("x".to_string())),
    ];


    // Bootstrap operations
    let forks = hashmap! {
        block_hash_a => vec![(block_hash_b.clone(), batch_1)],
        block_hash_b => vec![(block_hash_c.clone(), batch_2)]
    };

    runner(stf, fork_state_manager, chain, finalized, forks);

    let db = &db.lock().unwrap().data;
    for (k, v) in db {
        println!("K={}, V={}", k, v)
    }
}
