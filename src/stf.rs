use crate::state::State;

struct WorkingSet {
    a: usize
}

impl<S: State> From<S> for WorkingSet {
    fn from(_value: S) -> Self {
        Self {
            a: 0,
        }
    }
}


/// Process batch of operations and return new snapshot
pub fn stf<S: State>(base_snapshot: S, operations_count: usize) -> S {
    for batch_id in 0..3 {
        for i in 0..operations_count {


        }
    }
    base_snapshot
}