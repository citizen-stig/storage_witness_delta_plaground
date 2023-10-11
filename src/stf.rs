use std::hash::Hash;
use std::marker::PhantomData;
use sov_first_read_last_write_cache::cache::CacheLog;
use sov_first_read_last_write_cache::{CacheKey, CacheValue};
use crate::block_state_manager::TreeQuery;
use crate::db::Persistence;
use crate::rollup_interface::Snapshot;
use crate::state::{DB, FrozenSnapshot, StateCheckpoint};
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


pub struct SampleSTF<P: Persistence<Payload=CacheLog>, S: Snapshot<Id=Bh>, Bh> {
    state_root: u64,
    phantom_persistence: PhantomData<P>,
    phantom_snapshot: PhantomData<S>,
    phantom_bh: PhantomData<Bh>,
    // TODO: Should be read only db
    db: DB,
}

impl<P, S, Bh> SampleSTF<P, S, Bh>
    where
        P: Persistence<Payload=CacheLog>,
        S: Snapshot<Id=Bh, Key=CacheKey, Value=CacheValue>
{
    pub fn new(db: DB) -> Self {
        Self {
            state_root: 0,
            phantom_persistence: PhantomData,
            phantom_snapshot: PhantomData,
            phantom_bh: PhantomData,
            db,
        }
    }
}

//
impl<P, S, Bh> SampleSTF<P, S, Bh>
    where
        P: Persistence<Payload=CacheLog>,
        S: Snapshot<Id=Bh, Key=CacheKey, Value=CacheValue>,
        Bh: Eq + Hash + Clone,
{
    fn apply_operation(&mut self, checkpoint: StateCheckpoint<P, S, Bh>, operation: Operation) -> StateCheckpoint<P, S, Bh> {
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

impl<P, S, Bh> STF for SampleSTF<P, S, Bh>
    where
        P: Persistence<Payload=CacheLog>,
        S: Snapshot<Id=Bh, Key=CacheKey, Value=CacheValue>,
        Bh: Eq + Hash + Clone
{
    type StateRoot = u64;
    type Witness = Witness;
    type BlobTransaction = Operation;
    type CheckpointRef = TreeQuery<P, S, Bh>;
    type Snapshot = FrozenSnapshot<Bh>;

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

