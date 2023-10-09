/// Snapshot of the state
/// It can give a value that has been written/created on given state
/// It should not query parents or database
/// [`ForkTreeManager`] suppose to operate over those
pub trait Snapshot {
    type Key;
    type Value: Clone;
    type Id: Default + Copy;

    /// Get own value, value from its own cache
    fn get_value(&self, key: &Self::Key) -> Option<Self::Value>;

    /// Helper method for mapping
    fn get_id(&self) -> Self::Id;
}


/// Manages relations between forks of L1 and corresponding snapshots
/// Allows to add new snapshot or get reference to snapshot at given L1 bloc transition.
/// It is going to live under services/
pub trait ForkTreeManager {
    /// [`Snapshot`] effectively represents cache layer
    type Snapshot: Snapshot;
    /// Up to the implementors,
    /// but original idea is to give opportunity to query parent snapshots via this reference
    type SnapshotRef;
    /// Connection to `DaSpec`
    type BlockHash;

    /// Creates new snapshot reference for `current_block_hash` based on top of `prev_block_hash`
    /// Only 1 reference is allowed for give block hashes pair
    /// TODO: Replace with Result? Or just panic?
    fn get_new_ref(&mut self, prev_block_hash: &Self::BlockHash, current_block_hash: &Self::BlockHash) -> Self::SnapshotRef;

    /// Add new snapshot.
    /// Questions:
    ///     - If parent snapshot was discarded should we treat it special, or memory leak
    fn add_snapshot(&mut self, snapshot: Self::Snapshot);

    /// Cleans up chain graph and saves state associated with block hash, if present.
    /// Given block hash can be finalized only once
    ///
    fn finalize_snapshot(&mut self, block_hash: &Self::BlockHash);
}