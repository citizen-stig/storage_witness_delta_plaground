use std::marker::PhantomData;
use crate::state::StateTreeManager;
use crate::types::{Key, Value};

// use crate::state::StateCheckpoint;
// use crate::types::{Key, Value};
// use crate::witness::Witness;
//
pub trait STF {
    type StateRoot;
    type Witness;
    type BlobTransaction;

    type Checkpoint;


    fn apply_slot<'a, I>(
        &mut self,
        pre_state_root: &Self::StateRoot,
        base: Self::Checkpoint,
        blobs: I,
    ) ->
        (Self::StateRoot,
         Self::Witness,
         Self::Checkpoint)
        where
            I: IntoIterator<Item=Self::BlobTransaction>;
}

pub enum Operation {
    Get(Key),
    Set(Key, Value),
}


pub struct SampleSTF<Sm: StateTreeManager> {
    state_root: u64,
    phantom_sm: PhantomData<Sm>
}

impl<Sm: StateTreeManager> SampleSTF<Sm> {
    pub fn new() -> Self {
        Self {
            state_root: 0,
            phantom_sm: PhantomData,
        }
    }
}

//
// impl<Sm: StateTreeManager> SampleSTF<Sm> {
//     fn apply_operation(&mut self, base: SnapshotImpl<Sm>, operation: Operation) -> SnapshotImpl<Sm> {
//         // TODO: DB Comes from somewhere
//         let mut working_set = StateCheckpoint::new(DB::default(), base);
//         match operation {
//             Operation::Get(key) => {
//                 let value = working_set.get(&key);
//                 println!("Get {} {:?}", key.to_string(), value.map(|v| v.to_string()));
//             }
//             Operation::Set(key, value) => {
//                 let key_string = key.to_string();
//                 let value_string = value.to_string();
//                 if &key_string == "foo" && value_string == "bar" {
//                     println!("Skipping this transaction to previous state");
//                     return working_set.revert();
//                 }
//                 println!("Set {} = {}", key.to_string(), value.to_string());
//                 working_set.set(&key, value);
//             }
//         }
//         return working_set.commit();
//     }
// }

// impl STF for SampleSTF {
//     type StateRoot = u64;
//     type Witness = Witness;
//     type BlobTransaction = Operation;
//     type Checkpoint = StateCheckpoint;
//
//     fn apply_slot<'a, I>(&mut self, pre_state_root: &Self::StateRoot, base: Self::Checkpoint, blobs: I) -> (Self::StateRoot, Self::Witness, Self::Checkpoint) where I: IntoIterator<Item=Self::BlobTransaction> {
//         let mut checkpoint = base;
//         for operation in blobs {
//             checkpoint = self.apply_operation(checkpoint, operation);
//         }
//
//         (self.state_root, Witness::default(), checkpoint)
//     }
// }
