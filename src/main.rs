#![allow(dead_code)]
#![allow(unused_variables)]

mod db;
mod witness;
mod state;
mod block_state_manager;
mod stf;
mod types;

use std::sync::{Arc, Mutex};
use crate::block_state_manager::BlockStateManager;

use crate::db::{Database, Persistence};
use crate::state::{BlockStateSnapshot, FrozenStateCheckpoint};
use crate::stf::{SampleSTF, STF};

/// Requirements
///  - Consumers of "StateSnapshot" trait should be able to use it without knowing about manager and only returning one snapshot that needs to be committed.
///  - Consumers of "StateSnapshot" should not be able to persist snapshot(s) they are using
///  - SnapshotManager should be able to persist particular snapshot and it's parents, and invalidate all orphans


/// Requirement from rollup-interface
///  - Should make minimal restriction about implementation.
///  - Only should highlight what is expected from STF::apply_slot, basically to be stateless
///



/// Requirements from sov-modules-api / sov-state
///
/// - witness should be only tracked only for accesses outside of current snapshot
/// - snapshot should be able to correctly read data
///     from previous snapshot before resorting to the database
/// - snapshot should treat reading from previous snapshot as reading from database,
///     saving it in its own cache and updating witness
/// - whole machinery need to have type safety,
///     same as we use `WorkingSet::commit()` and `StateCheckpoint::to_revertable()`,
///     so we know when state is "clean"
///  - AppTemplate should be use this solution
///

fn runner<Stf, S, P>(stf: Stf, block_state_manager: BlockStateManager<S, P>)
    where
        S: BlockStateSnapshot,
        P: Persistence<Payload=S>,
        Stf: STF<Checkpoint=S::Checkpoint>,
        Stf::Checkpoint: Into<S>,
{
    todo!("")
}


fn main() {
    let db = Arc::new(Mutex::new(Database::default()));
    let stf = SampleSTF::new();
    let block_state_manager = BlockStateManager::<Arc<FrozenStateCheckpoint>, Database>::new(db.clone());
    runner(stf, block_state_manager)
}
