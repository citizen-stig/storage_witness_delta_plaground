use std::sync::{Arc, LockResult, RwLock, RwLockReadGuard};
use sov_first_read_last_write_cache::{CacheKey, CacheValue};



#[derive(Clone, Debug)]
pub struct Key {
    pub key: Arc<Vec<u8>>,
}


impl From<String> for Key {
    fn from(key: String) -> Self {
        Self {
            key: Arc::new(key.into_bytes()),
        }
    }
}

impl ToString for Key {
    fn to_string(&self) -> String {
        String::from_utf8_lossy(&self.key[..]).to_string()
    }
}

impl From<Key> for CacheKey {
    fn from(value: Key) -> Self {
        Self {
            key: value.key.clone(),
        }
    }
}

impl From<CacheKey> for Key {
    fn from(value: CacheKey) -> Self {
        Self {
            key: value.key.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Value {
    pub value: Arc<Vec<u8>>,
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self {
            value: Arc::new(value.into_bytes()),
        }
    }
}

impl From<Value> for String {
    fn from(value: Value) -> Self {
        String::from_utf8_lossy(&value.value[..]).to_string()
    }
}

impl ToString for Value {
    fn to_string(&self) -> String {
        String::from_utf8_lossy(&self.value[..]).to_string()
    }
}

impl From<Value> for CacheValue {
    fn from(value: Value) -> Self {
        Self {
            value: value.value.clone(),
        }
    }
}

impl From<CacheValue> for Value {
    fn from(value: CacheValue) -> Self {
        Self {
            value: value.value.clone(),
        }
    }
}


pub struct ReadOnlyLock<T> {
    lock: Arc<RwLock<T>>,
}

impl<T> ReadOnlyLock<T> {
    pub fn new(lock: Arc<RwLock<T>>) -> Self {
        Self {
            lock
        }
    }

    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, T>> {
        self.lock.read()
    }
}

