#![allow(dead_code)]
#![allow(unused_variables)]

use std::collections::HashMap;
use sov_first_read_last_write_cache::cache::CacheLog;
use crate::types::{Key, Value};

#[derive(Default, Debug)]
pub struct Database {
    data: HashMap<String, String>,
}

impl Database {
    pub fn get(&self, key: &str) -> Option<String> {
        self.data.get(key).cloned()
    }

    pub fn set(&mut self, key: &str, value: String) {
        self.data.insert(key.to_string(), value);
    }

    pub fn delete(&mut self, key: &str) {
        self.data.remove(key);
    }
}


pub fn persist_cache(db: &mut Database, cache: CacheLog) {
    let writes = cache.take_writes();
    for (key, value) in writes {
        let key = Key::from(key).to_string();
        match value {
            Some(value) => db.set(&key, Value::from(value).to_string()),
            None => db.delete(&key),
        }
    }
}