#![allow(dead_code)]
#![allow(unused_variables)]

use std::collections::HashMap;
use sov_first_read_last_write_cache::cache::CacheLog;

#[derive(Default, Debug)]
pub struct Database {
    pub data: HashMap<String, String>,
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


pub fn persist_cache(db: &mut Database, cache: CacheLog) {}


pub trait Storage {
    type Payload;
    fn commit(&mut self, data: Self::Payload);
}