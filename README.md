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

And STF has 2 new associated types `Snapshot` and `SnapshotRef`, without any bounds:

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


#### Full-node related

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

#### Module system related

New type `FronzenSnapshot` is just a holder of `CacheLog` and assigned `Id`. 
It implements `Snapshot` trait, as well as it can be saved to the database.
`BlockStateManager` operates on those.


### Modification of existing types

`StateCheckpoint` and its mutable pair `WorkingSet` now explicitly depend on `TreeQuery`.

it has same old `to_revertable` as well as `freeze` method which splits it into witness and FrozenSnapshot

```rust
pub struct StateCheckpoint<P: Persistence, S: Snapshot<Id=SnapshotId>, SnapshotId> {
    // For reading only
    db: DB,
    // Own cache
    cache: CacheLog,
    witness: Witness,
    parent: TreeQuery<P, S, SnapshotId>,
}

impl<P, S, SnapshotId> StateCheckpoint<P, S, SnapshotId>
    where
        P: Persistence,
        S: Snapshot<Id=SnapshotId, Key=CacheKey, Value=CacheValue>,
        SnapshotId: Eq + Hash + Clone
{
    pub fn new(db: DB, parent: TreeQuery<P, S, SnapshotId>) -> Self {
        todo!("simple assigning");
    }

    pub fn to_revertable(self) -> WorkingSet<P, S, SnapshotId> {
        WorkingSet {
            db: self.db,
            cache: RevertableWriter::new(self.cache),
            witness: self.witness,
            parent: self.parent,
        }
    }

    pub fn freeze(mut self) -> (Witness, FrozenSnapshot<S::Id>) {
        let witness = std::mem::replace(&mut self.witness, Default::default());
        let snapshot = FrozenSnapshot {
            id: self.parent.get_id(),
            local_cache: self.cache,
        };

        (witness, snapshot)
    }
}
```

`WorkingSet` is almost the same, but it has `get`/`set`/`delete` methods. 
`get` method uses `TreeQuery` to get value that wasn't saved to database yet:

```rust
pub struct WorkingSet<P: Persistence, S: Snapshot<Id=SnapshotId>, SnapshotId> {
    db: DB,
    cache: RevertableWriter<CacheLog>,
    witness: Witness,
    parent: TreeQuery<P, S, SnapshotId>,
}

impl<P, S, Bh> WorkingSet<P, S, Bh> {
    /// Public interface. Reads local cache, then tries parents and then database, if parent was committed
    pub fn get(&mut self, key: &Key) -> Option<Value> {
        let cache_key = CacheKey::from(key.clone());
        // Read from own cache
        let value = self.cache.inner.get(key);
        if value.is_some() {
            return value;
        }

        // Check parents or read database
        let cache_value = match self.parent.get_value_from_cache_layers(&cache_key) {
            Some(value) => Some(value),
            None => {
                let db = self.db.lock().unwrap();
                let db_key = key.to_string();
                db.get(&db_key).map(|v| CacheValue::from(Value::from(v)))
            }
        };

        // Save in own cache and log witness
        value
    }
}
```


## Challenges


1. `WorkingSet` has more generics now: `Snapshot` trait and `SnapshotId(BlockHash)`.
   1. Can they be defined inside Spec? 
2. How this setup will work inside ZK?
3. Optional: split storage trait into: Readable storage and `Storag: ReadableStorage` + writing things.