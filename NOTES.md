
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
    - That's not a requirement
- New Generic in WorkingSet and StateCheckpoint
  - Probably hide it behind C::Spec::Storage::S ??? 
  - Rollup shouldn't be aware about 
  - Preston: It is a little messy to throw ForkTreeManager as top level for app template / module system
      - Hide  
- How this design plays out with ZK
  - ZK Implementation for ForkManager always return none, so WorkingSet reads it from "db", and "db" will have the witness written in native.
- Preston: Name: ForkManager or ForkTreeManager?
  - ....
