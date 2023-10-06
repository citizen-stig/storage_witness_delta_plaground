// This is something that goes into STF, STF can build StateCheckpoint out of it.
// Other potential names:
// -> cache layer
pub trait Snapshot {
    type Key;
    type Value: Clone;
    type Id: Default + Copy;

    /// Get own value, value from its own cache
    fn get_value(&self, key: &Self::Key) -> Option<Self::Value>;

    /// Helper method for mapping
    fn get_id(&self) -> Self::Id;
}


// This is something that managers parent/child relation between snapshots and according block hashes
// Potential other names:
// - ForkManager
// NOTE: DO we need this trait at all? Can we sov-runner use concrete implementation?
// Currently it is bound to CacheLayer
// It is going to live under services/
pub trait ForkTreeManager {
    type Snapshot: Snapshot;
    // Snapshot ref is capable to query data in all ancestor
    type SnapshotRef;
    type BlockHash;

    /// Creates new snapshot on top of block_hash
    /// If block hash is not present, consider it as genesis
    fn get_from_block(&mut self, block_hash: &Self::BlockHash) -> Self::SnapshotRef;

    /// Adds new snapshot with given block hash to the chain
    /// Implementation is responsible for maintaining connection between block hashes
    /// NOTE: Maybe we don't need parent, and find parent hash from snapshot id?
    fn add_snapshot(&mut self,
                    parent_block_hash: &Self::BlockHash,
                    block_hash: &Self::BlockHash,
                    snapshot: Self::Snapshot);

    /// Cleans up chain graph and saves state associated with block hash, if present
    fn finalize_snapshot(&mut self, block_hash: &Self::BlockHash);
}