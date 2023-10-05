# Sandbox for simulating snapshot based STF with Reorgs

## Types

### sovereign-sdk simplified types:

* `Key`, `Value` : simplified version of StorageKey and StorageValue 

### New Traits

We assume to have 2 new traits: Snapshot and ForkTreeManager

```rust
pub trait Snapshot {
    type Key;
    type Value: Clone;
    type Id: Default + Copy;

    /// Get own value, value from its own cache
    fn get_value(&self, key: &Self::Key) -> Option<Self::Value>;

    /// Helper method for mapping
    fn get_id(&self) -> Self::Id;
}

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
```

Only *`Snapshot`* will be visible for STF. 

`ForkTreeManager` is going to live under "node/services". This will allow to have stf-runner remain generic enough to not depend on module system, but only on iterface


### New Types



### Modification of existing types

