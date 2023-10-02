use std::sync::{Arc, Mutex};
use sov_first_read_last_write_cache::cache::CacheLog;
use crate::db::Database;

pub type DB = Arc<Mutex<Database>>;

pub trait StateSnapshot {
    fn on_top(&self) -> Self;

    fn commit(&self) -> CacheLog;
}

pub struct WorkingSet {
    db: DB,
    cache: CacheLog,
    parent: Option<Arc<StateCheckpoint>>
}

impl WorkingSet {

}

pub struct StateCheckpoint {}