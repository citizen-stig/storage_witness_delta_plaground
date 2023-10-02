#![allow(dead_code)]
#![allow(unused_variables)]

use std::cell::RefCell;
use crate::types::{Key, Value};

#[derive(Default, Debug)]
pub struct Witness {
    data: RefCell<Vec<(Key, Option<Value>)>>,
}

impl Witness {
    pub fn track_operation(&self, key: &Key, value: Option<Value>) {
        self.data.borrow_mut().push((key.clone(), value));
    }

    pub fn len(&self) -> usize {
        self.data.borrow().len()
    }
}
