use crate::state::State;

/// Process batch of operations and return new snapshot
fn stf<S: State>(base_snapshot: S, operations_count: usize) -> S {
    for _batch_id in 0..3 {
        for _i in 0..operations_count {
            // DO SOMETHING
        }
    }
    base_snapshot
}