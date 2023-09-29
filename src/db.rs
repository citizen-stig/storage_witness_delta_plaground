#![allow(dead_code)]
#![allow(unused_variables)]
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct Database {
    data: HashMap<String, u64>,
}

impl Database {
    pub fn get(&self, key: &str) -> Option<u64> {
        self.data.get(key).copied()
    }

    pub fn set(&mut self, key: &str, value: u64) {
        self.data.insert(key.to_string(), value);
    }

    pub fn delete(&mut self, key: &str) {
        self.data.remove(key);
    }

    pub fn persist(&mut self, operations: DbOperations) {
        for (key, operation) in operations {
            match operation {
                None => {
                    self.data.remove(&key);
                }
                Some(data) => {
                   self.data.insert(key, data);
                }
            }
        }
    }
}



pub type DbOperations = HashMap<String, Option<u64>>;