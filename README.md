# Sandbox for simulating snapshot-based STF with Reorgs

## Types

### sovereign-sdk simplified types:

* `Key`, `Value` : simplified version of StorageKey and StorageValue

### New Traits

We assume to have a new trait

```rust
pub trait Snapshot {
    type Id: Clone;
    type Key;
    type Value: Clone;

    fn get_value(&self, key: &Self::Key) -> Option<Self::Value>;

    fn get_id(&self) -> &Self::Id;
}
```

And STF has 2 new associated types `Snapshot` and `SnapshotRef`:

```rust
pub trait STF {
    type Witness;
    type BlobTransaction;

    type SnapshotRef;
    type Snapshot;


    fn apply_slot<'a, I>(
        &mut self,
        base: Self::SnapshotRef,
        blobs: I,
    ) -> (Self::Witness, Self::Snapshot)
        where
            I: IntoIterator<Item=Self::BlobTransaction>;
}

```

### New Types

`BlockStateManager` is a concrete implementation that handles relationship between snapshots

```
pub struct BlockStateManager<P: Persistence, S: Snapshot<Id=Bh>, Bh> {
    // Storage
    db: Arc<Mutex<P>>,

    snapshots: HashMap<Bh, S>,

    // L1 forks representation
    // Chain: prev_block -> child_blocks
    chain_forks: HashMap<Bh, Vec<Bh>>,
    // Reverse: child_block -> parent
    blocks_to_parent: HashMap<Bh, Bh>,
}
```

It includes:

1. Produce snapshot reference for given block transition.
2. Query value for given snapshot up to all unsaved parents
3. Save and later finalize snapshots

When "new snapshot" is requested, BlockStateManager produces a struct that holds reference to BlockStateManager itself, 
and id that allows to identify it in a tree and perform value fetching:

```rust
pub struct TreeQuery<P, S, Bh>
    where
        P: Persistence,
        S: Snapshot<Id=Bh>
{
    pub id: Bh,
    pub manager: ReadOnlyLock<BlockStateManager<P, S, Bh>>,
}


// Trait bounds are simplified
impl<P, S, Bh> TreeQuery<P, S, Bh> {
    pub fn get_value_from_cache_layers(&self, key: &S::Key) -> Option<S::Value> {
        let manager = self.manager.read().unwrap();
        manager.get_value_recursively(&self.id, key)
    }
}

impl BlockStateManager {
    pub fn get_new_ref(&mut self, prev_block_hash: &Bh, current_block_hash: &Bh) -> TreeQuery<P, S, Bh> {
        todo!("implementation elided")
    }

    pub fn get_value_recursively(&self, snapshot_block_hash: &S::Id, key: &S::Key) -> Option<S::Value> {
        todo!("traverses tree backwards, querying each snapshot")
    }
}
```


### Modification of existing types




