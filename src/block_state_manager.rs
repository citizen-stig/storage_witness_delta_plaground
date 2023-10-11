use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, RwLock};
use sov_first_read_last_write_cache::{CacheKey, CacheValue};
use crate::db::Persistence;
use crate::rollup_interface::{Snapshot};
use crate::state::{FrozenSnapshot};
use crate::types::{Key, ReadOnlyLock, Value};

pub type BlockHash = String;

pub struct TreeQuery<P, S, Bh>
    where
        P: Persistence,
        S: Snapshot<Id=Bh>
{
    pub id: Bh,
    pub manager: ReadOnlyLock<BlockStateManager<P, S, Bh>>,
}


impl<P, S, Bh> TreeQuery<P, S, Bh>
    where
        P: Persistence,
        S: Snapshot<Id=Bh>
{
    pub fn new(id: S::Id, manager: ReadOnlyLock<BlockStateManager<P, S, Bh>>) -> Self {
        Self {
            id,
            manager,
        }
    }
}

impl<P, S, Bh> Snapshot for TreeQuery<P, S, Bh>
    where
        P: Persistence,
        S: Snapshot<Id=Bh> + Into<P::Payload>,
        Bh: Eq + Hash + Clone
{
    type Id = Bh;
    type Key = S::Key;
    type Value = S::Value;

    /// Queries value recursively from associated manager
    fn get_value(&self, key: &Self::Key) -> Option<Self::Value> {
        let manager = self.manager.read().unwrap();
        manager.get_value_recursively(&self.id, key)
    }

    fn get_id(&self) -> &Self::Id {
        &self.id
    }
}


#[derive(Debug)]
pub struct BlockStateManager<P: Persistence, S: Snapshot<Id=Bh>, Bh> {
    // Storage
    db: Arc<Mutex<P>>,
    // Helpers
    self_ref: Option<Arc<RwLock<BlockStateManager<P, S, Bh>>>>,

    // Snapshots
    // snapshot_id => snapshot
    snapshots: HashMap<Bh, S>,

    // L1 forks representation
    // Chain: prev_block -> child_blocks (forks
    chain_forks: HashMap<Bh, Vec<Bh>>,
    // Reverse: child_block -> parent
    blocks_to_parent: HashMap<Bh, Bh>,

}

impl<P, S, Bh> BlockStateManager<P, S, Bh>
    where
        P: Persistence,
        S: Snapshot<Id=Bh> + Into<P::Payload>,
        Bh: Eq + Hash + Clone
{
    pub fn new_locked(db: Arc<Mutex<P>>) -> Arc<RwLock<Self>> {
        let block_state_manager = Arc::new(RwLock::new(Self {
            db,
            chain_forks: Default::default(),
            blocks_to_parent: Default::default(),
            snapshots: Default::default(),
            self_ref: None,
        }));
        let self_ref = block_state_manager.clone();
        {
            let mut bm = block_state_manager.write().unwrap();
            bm.self_ref = Some(self_ref);
        }
        block_state_manager
    }

    pub fn get_value_recursively(&self, snapshot_block_hash: &S::Id, key: &S::Key) -> Option<S::Value> {
        let parent_block_hash = self.blocks_to_parent.get(snapshot_block_hash)?;
        let mut parent_snapshot = self.snapshots.get(parent_block_hash);
        while parent_snapshot.is_some() {
            let snapshot = parent_snapshot.unwrap();
            let value = snapshot.get_value(key);
            if value.is_some() {
                return value;
            }
            let parent_block_hash = self.blocks_to_parent.get(snapshot.get_id())?;
            parent_snapshot = self.snapshots.get(parent_block_hash);
        }
        None
    }

    pub fn stop(mut self) {
        self.self_ref = None
    }

    pub fn is_empty(&self) -> bool {
        self.chain_forks.is_empty()
            && self.blocks_to_parent.is_empty()
            && self.snapshots.is_empty()
    }

    pub fn get_new_ref(&mut self, prev_block_hash: &Bh, current_block_hash: &Bh) -> TreeQuery<P, S, Bh> {
        let new_snapshot_ref = TreeQuery {
            id: current_block_hash.clone(),
            manager: ReadOnlyLock::new(self.self_ref.clone().unwrap().clone()),
        };


        let c = self.blocks_to_parent.insert(current_block_hash.clone(), prev_block_hash.clone());
        // TODO: Maybe assert that parent is the same? Then
        assert!(c.is_none(), "current block hash has already snapshot requested");
        self.chain_forks.entry(prev_block_hash.clone()).or_insert(Vec::new()).push(current_block_hash.clone());

        new_snapshot_ref
    }

    pub fn add_snapshot(&mut self, snapshot: S) {
        self.snapshots.insert(snapshot.get_id().clone(), snapshot);
    }

    pub fn finalize_snapshot(&mut self, block_hash: &Bh) {
        let snapshot = self.snapshots.remove(&block_hash).expect("Tried to finalize non-existing snapshot: self.snapshots");
        let payload = snapshot.into();
        {
            let mut db = self.db.lock().unwrap();
            db.commit(payload);
        }


        if let Some(parent_block_hash) = self.blocks_to_parent.remove(block_hash) {
            let mut to_discard: Vec<_> = self.chain_forks.remove(&parent_block_hash).expect("Inconsistent chain_forks")
                .into_iter()
                .filter(|bh| bh != block_hash)
                .collect();
            while !to_discard.is_empty() {
                let next_to_discard = to_discard.pop().unwrap();
                let next_children_to_discard = self.chain_forks.remove(&next_to_discard).unwrap_or(Default::default());
                to_discard.extend(next_children_to_discard);

                if let Some(_snapshot) = self.snapshots.remove(&next_to_discard) {
                    self.blocks_to_parent.remove(&next_to_discard).unwrap();
                    self.snapshots.remove(&next_to_discard).unwrap();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::db::Database;
    use crate::state::{DB, StateCheckpoint};
    use super::*;

    fn write_values(db: DB, snapshot_ref: TreeQuery<Database, FrozenSnapshot<BlockHash>, BlockHash>, values: &[(&str, &str)]) -> FrozenSnapshot<BlockHash> {
        let checkpoint = StateCheckpoint::new(db.clone(), snapshot_ref);
        let mut working_set = checkpoint.to_revertable();
        for (key, value) in values {
            let key = Key::from(key.to_string());
            let value = Value::from(value.to_string());
            working_set.set(&key, value);
        }
        let checkpoint = working_set.commit();
        let (_witness, snapshot) = checkpoint.freeze();
        snapshot
    }

    mod fork_tree_manager {
        use super::*;

        #[test]
        fn new() {
            let db = DB::default();
            let state_manager = BlockStateManager::<Database, FrozenSnapshot<BlockHash>, BlockHash>::new_locked(db.clone());
            let state_manager = state_manager.write().unwrap();
            assert!(state_manager.self_ref.is_some());
            {
                let db = db.lock().unwrap();
                assert!(db.data.is_empty());
            }
            assert!(state_manager.is_empty());
        }


        #[test]
        fn linear_progression_with_2_blocks_delay() {
            let db = DB::default();
            let state_manager = BlockStateManager::new_locked(db.clone());
            let mut state_manager = state_manager.write().unwrap();
            assert!(state_manager.is_empty());
            let genesis_block = "genesis".to_string();
            let block_a = "a".to_string();
            let block_b = "b".to_string();
            let block_c = "c".to_string();

            // Block A
            let block_a_values = vec![
                ("x", "1"),
                ("y", "2"),
            ];
            let snapshot_ref = state_manager.get_new_ref(&genesis_block, &block_a);

            assert!(!state_manager.is_empty());
            let snapshot = write_values(db.clone(), snapshot_ref, &block_a_values);
            state_manager.add_snapshot(snapshot);
            assert!(!state_manager.is_empty());
            {
                assert!(db.lock().unwrap().data.is_empty());
            }

            // Block B
            let block_b_values = vec![
                ("x", "3"),
                ("z", "4"),
            ];
            let snapshot_ref = state_manager.get_new_ref(&block_a, &block_b);
            let snapshot = write_values(db.clone(), snapshot_ref, &block_b_values);
            let snapshot_id_b = snapshot.get_id().clone();
            state_manager.add_snapshot(snapshot);
            assert_eq!(Some(CacheValue::from(Value::from("1".to_string()))), state_manager.get_value_recursively(&snapshot_id_b, &CacheKey::from(Key::from("x".to_string()))));
            {
                assert!(db.lock().unwrap().data.is_empty());
            }
            println!("AFTER B: {:?}", state_manager);
            // Finalizing A
            state_manager.finalize_snapshot(&block_a);
            {
                let db = db.lock().unwrap();
                assert!(!db.data.is_empty());
                assert_eq!(Some("1".to_string()), db.get("x"));
                assert_eq!(Some("2".to_string()), db.get("y"));
                assert_eq!(None, db.get("z"));
            }
            println!("AFTER FINALIZING A: {:?}", state_manager);

            // Block C
            let block_c_values = vec![
                ("x", "5"),
                ("z", "6"),
            ];
            let snapshot_ref = state_manager.get_new_ref(&block_b, &block_c);
            let snapshot = write_values(db.clone(), snapshot_ref, &block_c_values);
            state_manager.add_snapshot(snapshot);
            println!("AFTER C: {:?}", state_manager);
            // Finalizing B
            state_manager.finalize_snapshot(&block_b);
            assert!(!state_manager.is_empty());
            {
                let db = db.lock().unwrap();
                assert!(!db.data.is_empty());
                assert_eq!(Some("3".to_string()), db.get("x"));
                assert_eq!(Some("2".to_string()), db.get("y"));
                assert_eq!(Some("4".to_string()), db.get("z"));
            }

            state_manager.finalize_snapshot(&block_c);
            // TODO: Finalize everything, it should be clean
            println!("AFTER FINALIZING C: {:?}", state_manager);
            assert!(state_manager.is_empty());
        }

        #[test]
        #[ignore = "TBD"]
        fn fork_added() {}

        #[test]
        #[ignore = "TBD"]
        fn adding_alien_snapshot() {}

        #[test]
        #[ignore = "TBD"]
        fn finalizing_alien_block() {}

        #[test]
        #[ignore = "TBD"]
        fn finalizing_same_block_hash_twice() {}

        #[test]
        #[ignore = "TBD"]
        fn requesting_ref_from_same_block_twice() {}
    }
}