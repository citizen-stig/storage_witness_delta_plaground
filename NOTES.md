

///


/// Notes:
/ 0. Use Weak to parent
  1. Read from DB not in the "latest" node in chain,
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


Whoever constructs STF and BlockStateManager,
need to add constraint to convert StateCheckpoint back to StateSnapshot

