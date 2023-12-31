# Older

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
///

# 02.10

1. Use Weak to parent
2. Read from DB not in the "latest" node in chain,
     but in the oldest not committed, by checking if Weak<Parent> is present
    Do not use snapshoting inside STF, but just modereate...

We don't want STF to be able to spawn children, so weak ref will be dropped
So we want StateSnapshot to produce StateCheckpoint(associated type, concrete)
and then give it to STF. Stf returns it back, effectively, also plays with WorkingSet.

Questions:
  - How to convert StateCheckpoint back to StateSnapshot?
  - How ForkManager knows how to commit StateCheckpoint?

BlockStateManager<S: StateSnapshot>:: -> S::WorkingSetLike
Stf(S::WorkingSetLike) -> S::WorkingSetLike.
BlockStateManager::add_snapshot(block_hash, s: S)

DB needs to be generic, and this associated type,

Whoever constructs STF and BlockStateManager, need to add constraint to convert StateCheckpoint back to StateSnapshot


# 03.10


Questions

 - What is the relation between existing StateCheckpoint and WorkingSet, as they do not commit themself anymore? Would it be just cache collapse or revert?
   - WorkingSet is effectively Cache of internal cache, so it only writes
 - Why the earliest node needs to read from DB? Why cannot it be done in the latest?
   - Q from Blaze: Why Frozen and StateCheckpoint both need database instance?
 - How to express that S is Arc<payload>, so it is BlockStateManager who can do try_from?
 - 


TODO:
 - Add working example of STF
 - Remove duplication in Frozen or StateCheckpoint

# 04.10

Questions

- Why the earliest node needs to read from DB? Why cannot it be done in the latest?
    - Q from Blaze: Why Frozen and StateCheckpoint both need database instance?
    - A: That's not a requirement
- New Generic in WorkingSet and StateCheckpoint
  - Probably hide it behind C::Spec::Storage::S ??? 
  - Rollup shouldn't be aware about 
  - Preston: It is a little messy to throw ForkTreeManager as top level for app template / module system
      - Hide  
- How this design plays out with ZK
  - ZK Implementations for ForkManager always return none, so WorkingSet reads it from "db", and "db" will have the witness written in native.
- Preston: Name: ForkManager or ForkTreeManager?
  - ....


# 05.10

 - Do another round of traits API simplification, if it is possible.
 - Think a little more about how ForkTreeManager is going to fit into sov-runner and/or sov-rollup-interface
 - Q: Blazej: Do new traits make sense without the module system?
 - Think how `ForkTreeManager` fits into ZK
 - Q: Nikolai: What is the difference between S::Witness and CacheLog tracking reads?


# 06.10

 - +Do another round of traits API simplification, if it is possible.
    - Write tests to see edge cases
 - Remove extra method parameters from ForkTreeManager
 - Idea: Try to make Snapshot generic for snapshot ref instead of concrete implementation.
 - + Do we need ForkTreeManager trait at all? Can we sov-runner use concrete implementation?
   - Yes. sov-stf-runner should be generic and do not depend on concrete implementation

# 09.10

 - Snapshot ID: can we stick to concrete type, like u64
   - Yes. at a rate of 3 snapshot IDs per second, it would take approximately 194,810,567 years for the u64 ID to overflow.
 - Can BlockStateManager be a concrete type generic over something
   - Yes, by overusing Snapshot Trait

# 10.10
 - Can Snapshot use block hash as id? So it will be generic for sov-state, but actual block hash?
 - Blazej: Can we remove generic from StateCheckpoint, 
   - It becomes generic over BlockHash, which lives in DaSpec and Context does not have access to id
     - Maybe use Vec<u8>
     - Passing DaSpec inside Spec
 - Self_ref in BlockStateManager can be removed and use just wrapper Type
   - 


TODO:
 + Make Snapshot ID generic
+ Replace SnapshotID with BlockHash in StateManager


# 11.10

 - Should Snapshot::Id bounds of `Eq + Hash + Clone` be on trait or added on required impls?
 - Is it better to do "dancing" with 2 implementation of Snapshot traits, rather than explicitly depend on `BlockStateManager`
   - What about "native" then?
 - Should we use `ParentsLookup` trait and block state manager just implements it?
 - Where `Snapshot` trait should live? Can it live in `BlockStateManager` trait? What is advantage of having it in
 - 