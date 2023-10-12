use std::hash::Hash;
use std::marker::PhantomData;
use sov_first_read_last_write_cache::cache::CacheLog;
use sov_first_read_last_write_cache::{CacheKey, CacheValue};
use crate::block_state_manager::{BlockStateManager, TreeQuery};
use crate::db::Storage;
use crate::rollup_interface::STF;
use crate::state::{DB, FrozenSnapshot, StateCheckpoint};
use crate::types::{Key, Value};
use crate::witness::Witness;


pub enum Operation {
    Get(Key),
    Set(Key, Value),
}


pub struct SampleSTF<P: Storage<Payload=CacheLog>, Bh> {
    phantom_persistence: PhantomData<P>,
    phantom_bh: PhantomData<Bh>,
    // TODO: Should be read only db
    db: DB,
}

impl<P, Bh> SampleSTF<P, Bh>
    where
        P: Storage<Payload=CacheLog, Key=CacheKey, Value=CacheValue>,
{
    pub fn new(db: DB) -> Self {
        Self {
            phantom_persistence: PhantomData,
            phantom_bh: PhantomData,
            db,
        }
    }
}

//
impl<P, Bh> SampleSTF<P, Bh>
    where
        P: Storage<Payload=CacheLog, Key=CacheKey, Value=CacheValue>,
        Bh: Eq + Hash + Clone,
{
    fn apply_operation(&mut self, checkpoint: StateCheckpoint<P, BlockStateManager<P, FrozenSnapshot, Bh>>, operation: Operation) -> StateCheckpoint<P, BlockStateManager<P, FrozenSnapshot, Bh>> {
        let mut working_set = checkpoint.into_revertable();
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


impl<P, Bh> STF for SampleSTF<P, Bh>
    where
        P: Storage<Payload=CacheLog, Key=CacheKey, Value=CacheValue>,
        Bh: Eq + Hash + Clone
{
    type Witness = Witness;
    type BlobTransaction = Operation;
    type SnapshotRef = TreeQuery<P, BlockStateManager<P, FrozenSnapshot, Bh>>;
    type ChangeSet = FrozenSnapshot;

    fn apply_slot<'a, I>(&mut self, base: Self::SnapshotRef, blobs: I) -> (Self::Witness, Self::ChangeSet) where I: IntoIterator<Item=Self::BlobTransaction> {
        let mut checkpoint = StateCheckpoint::new(base);
        for operation in blobs {
            checkpoint = self.apply_operation(checkpoint, operation);
        }

        let (witness, snapshot) = checkpoint.freeze();

        (witness, snapshot)
    }
}

