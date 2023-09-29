#![allow(dead_code)]
#![allow(unused_variables)]
use std::collections::HashMap;
use crate::state::{Checkpoint};

type SnapshotId = u64;
type BlockHash = String;


struct SnapshotManager {
    last_snapshot_id: SnapshotId,
    chain: HashMap<SnapshotId, Vec<SnapshotId>>,
    snapshots: HashMap<SnapshotId, Checkpoint>,
    current: SnapshotId,
    block_snaps: HashMap<BlockHash, SnapshotId>
}
