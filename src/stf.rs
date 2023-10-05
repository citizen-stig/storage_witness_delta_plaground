use std::marker::PhantomData;
use sov_first_read_last_write_cache::cache::CacheLog;
use crate::db::Persistence;
use crate::state::{DB, FrozenSnapshot, TreeManagerSnapshotQuery, StateCheckpoint};
use crate::types::{Key, Value};
use crate::witness::Witness;

pub trait STF {
    type StateRoot;
    type Witness;
    type BlobTransaction;

    type CheckpointRef;
    type Snapshot;


    fn apply_slot<'a, I>(
        &mut self,
        base: Self::CheckpointRef,
        blobs: I,
    ) ->
        (Self::StateRoot,
         Self::Witness,
         Self::Snapshot)
        where
            I: IntoIterator<Item=Self::BlobTransaction>;
}

pub enum Operation {
    Get(Key),
    Set(Key, Value),
}


pub struct SampleSTF<P: Persistence<Payload=CacheLog>> {
    state_root: u64,
    phantom_sm: PhantomData<P>,
    // TODO: Should be read only db
    db: DB,
}

impl<P: Persistence<Payload=CacheLog>> SampleSTF<P> {
    pub fn new(db: DB) -> Self {
        Self {
            state_root: 0,
            phantom_sm: PhantomData,
            db,
        }
    }
}

//
impl<P: Persistence<Payload=CacheLog>> SampleSTF<P> {
    fn apply_operation(&mut self, checkpoint: StateCheckpoint<P>, operation: Operation) -> StateCheckpoint<P> {
        let mut working_set = checkpoint.to_revertable();
        match operation {
            Operation::Get(key) => {
                let value = working_set.get(&key);
                println!("Get {} {:?}", key.to_string(), value.map(|v| v.to_string()));
            }
            Operation::Set(key, value) => {
                let key_string = key.to_string();
                let value_string = value.to_string();
                // TODO: First try to read existing value, so we have a case of non polluting reads
                if &key_string == "foo" && value_string == "bar" {
                    println!("Skipping this transaction to previous state");
                    return working_set.revert();
                }
                println!("Set {} = {}", key.to_string(), value.to_string());
                working_set.set(&key, value);
            }
        }
        return working_set.commit();
    }
}

impl<P: Persistence<Payload=CacheLog>> STF for SampleSTF<P> {
    type StateRoot = u64;
    type Witness = Witness;
    type BlobTransaction = Operation;
    type CheckpointRef = TreeManagerSnapshotQuery<P>;
    type Snapshot = FrozenSnapshot;

    fn apply_slot<'a, I>(&mut self, base: Self::CheckpointRef, blobs: I) -> (Self::StateRoot, Self::Witness, Self::Snapshot) where I: IntoIterator<Item=Self::BlobTransaction> {
        let mut checkpoint = StateCheckpoint::new(self.db.clone(), base);
        for operation in blobs {
            checkpoint = self.apply_operation(checkpoint, operation);
        }

        let (witness, snapshot) = checkpoint.freeze();
        let state_root = self.state_root;

        (state_root, witness, snapshot)
    }
}

