#![allow(dead_code)]
#![allow(unused_variables)]

use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::sync::{Arc, Mutex, RwLock};
use crate::block_state_manager::{BlockHash, BlockStateManager, TreeQuery};
use crate::db::{Database, Persistence};
use crate::rollup_interface::{Snapshot, STF};
use crate::state::{FrozenSnapshot};
use crate::stf::{Operation, SampleSTF};
use crate::types::{Key, Value};

mod db;
mod witness;
mod state;
mod block_state_manager;
mod stf;
mod types;
mod rollup_interface;

fn runner<Stf, P, S, B, Bh>(
    mut stf: Stf,
    fork_manager: Arc<RwLock<BlockStateManager<P, S, Bh>>>,
    // Simulates arrival of DA blocks
    chain: Vec<Bh>,
    // Matches length of chain, and when value is present this block is
    finalized_blocks: Vec<Option<Bh>>,
    // Simulates what forks are created at given parent block
    mut batches: HashMap<Bh, Vec<(Bh, Vec<B>)>>)
    where
    // This constraint is for a map.
        Bh: Eq + Hash + Clone + Display,
        P: Persistence,
        S: Snapshot<Id=Bh> + Into<P::Payload>,
        Stf: STF<BlobTransaction=B, Snapshot=S, SnapshotRef=TreeQuery<P, S, Bh>>,

{
    assert_eq!(chain.len(), finalized_blocks.len());
    for (current_block_hash, finalized_block_hash) in chain.into_iter().zip(finalized_blocks.into_iter()) {
        println!("== Iterating over current block {}", current_block_hash);
        let forks = batches.remove(&current_block_hash).unwrap_or_default();
        for (child_block_hash, blob) in forks {
            println!("Executing fork from prev={} to next={}", current_block_hash, child_block_hash);
            let snapshot_ref = {
                let mut fm = fork_manager.write().unwrap();
                fm.get_new_ref(&current_block_hash, &child_block_hash)
            };
            let (_witness, snapshot) = stf.apply_slot(snapshot_ref, blob);
            {
                let mut fm = fork_manager.write().unwrap();
                fm.add_snapshot(snapshot);
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
    let stf: SampleSTF<Database, FrozenSnapshot<BlockHash>, BlockHash> = SampleSTF::new(db.clone());

    // Bootstrap fork_state_manager
    let fork_state_manager = BlockStateManager::new_locked(db.clone());


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
        Operation::Set(Key::from("x".to_string()), Value::from("3".to_string())),
    ];
    let batch_3 = vec![
        Operation::Set(Key::from("x".to_string()), Value::from("4".to_string())),
        Operation::Get(Key::from("x".to_string())),
    ];


    // Bootstrap operations
    let forks = hashmap! {
        block_hash_a => vec![(block_hash_b.clone(), batch_1)],
        block_hash_b => vec![(block_hash_c.clone(), batch_2)],
        block_hash_c => vec![(block_hash_d.clone(), batch_3)]
    };

    runner(stf, fork_state_manager, chain, finalized, forks);

    let db = &db.lock().unwrap().data;
    for (k, v) in db {
        println!("K={}, V={}", k, v)
    }


    // Desired Chain:
    //       /-> g
    // a -> b -> c -> d -> e
    //  \-> e -> f -> h
    //       \-> k
}

