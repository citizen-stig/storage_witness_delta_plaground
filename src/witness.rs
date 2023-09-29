use std::cell::RefCell;

#[derive(Default)]
pub struct Witness {
    data: RefCell<Vec<(String, Option<u64>)>>,
}

impl Witness {
    pub fn track_operation(&self, key: &str, value: Option<u64>) {
        self.data.borrow_mut().push((key.to_string(), value));
    }
}