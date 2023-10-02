use crate::state::{DB, StateCheckpoint, StateSnapshot, WorkingSet};
use crate::types::{Key, Value};


pub trait STF {
    type StateRoot;
    type Witness;
    type BlobTransaction;

    type WorkingSet;




    fn apply_slot<'a, I>(
        &mut self,
        pre_state_root: &Self::StateRoot,
        blobs: I,
    ) ->
        (Self::StateRoot,
         Self::Witness,
         Into<StateSnapshot>)
        where
            I: IntoIterator<Item=Self::BlobTransaction>;
}


enum Operation {
    Get(Key),
    Set(Key, Value),
}

struct SampleSTF {
    state_root: u64,
    db: DB,
}

impl SampleSTF {
    pub fn new(db: DB) -> Self {
        Self {
            state_root: 0,
            db,
        }
    }
}

impl SampleSTF {
    fn apply_operation(&mut self, base: StateCheckpoint, operation: Operation) -> StateCheckpoint {
        let mut working_set = base.to_revertable();
        match operation {
            Operation::Get(key) => {
                let value = working_set.get(&key);
                println!("Get {} {:?}", key.to_string(), value.map(|v| v.to_string()));
            }
            Operation::Set(key, value) => {
                let key_string = key.to_string();
                let value_string = value.to_string();
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

impl STF for SampleSTF {
    type StateRoot = u64;
    type Witness = Vec<u64>;
    type BlobTransaction = Operation;

    fn apply_slot<'a, I>(&mut self, pre_state_root: &Self::StateRoot, blobs: I) -> (Self::StateRoot, Self::Witness) where I: IntoIterator<Item=Self::BlobTransaction> {
        let working_set = WorkingSet::new(self.db.clone());
        let mut checkpoint = working_set.commit();

        for operation in blobs {
            checkpoint = self.apply_operation(checkpoint, operation);
        }
        (self.state_root, vec![self.state_root])
    }
}
