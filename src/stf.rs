use std::collections::HashMap;
use crate::Operation;


pub trait STF {
    type StateRoot;
    type Witness;
    type BlobTransaction;


    fn apply_slot<'a, I>(
        &mut self,
        pre_state_root: &Self::StateRoot,
        blobs: I,
    ) ->
        (Self::StateRoot,
         Self::Witness)
        where
            I: IntoIterator<Item=Self::BlobTransaction>;
}


struct SampleSTF {
    state_root: u64,
    state: HashMap<String, u64>,
}

impl STF for SampleSTF {
    type StateRoot = u64;
    type Witness = Vec<u64>;
    type BlobTransaction = Operation;

    fn apply_slot<'a, I>(&mut self, pre_state_root: &Self::StateRoot, blobs: I) -> (Self::StateRoot, Self::Witness) where I: IntoIterator<Item=Self::BlobTransaction> {
        let mut prev_state_root = self.state_root;
        for operation in blobs {
            match operation {
                Operation::Get(key) => {
                    println!("Get {}", key);
                    prev_state_root += 1;
                }
                Operation::Set(key, value) => {
                    println!("Set {} = {}", key, value);
                    prev_state_root += 3;
                }
                Operation::Delete(delete) => {
                    println!("Delete {}", delete);
                    prev_state_root += 3
                }
            }
        }
        (prev_state_root, vec![prev_state_root])
    }
}
