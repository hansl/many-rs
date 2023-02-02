use crate::storage::KvStore;
use many_error::ManyError;
use sha3::{Digest, Sha3_256};
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct MemoryStorage {
    // This is a refcell as it needs to be replaced in hash() which doesn't have mutable rights.
    hash: RefCell<Option<Vec<u8>>>,
    inner: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl MemoryStorage {
    pub fn insert(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.inner.insert(key, value);
    }
}

impl KvStore for MemoryStorage {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ManyError> {
        Ok(self
            .inner
            .get(key)
            .map(|x| x.as_slice())
            .map(|v| v.to_vec()))
    }

    fn set(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), ManyError> {
        self.hash.replace(None);
        self.inner.insert(key, value);
        Ok(())
    }

    fn del(&mut self, key: &[u8]) -> Result<(), ManyError> {
        self.hash.replace(None);
        self.inner.remove(key);
        Ok(())
    }

    fn contains(&self, key: &[u8]) -> bool {
        self.inner.contains_key(key)
    }

    fn size(&self, key: &[u8]) -> Option<usize> {
        Some(self.inner.get(key)?.len())
    }

    fn hash(&self) -> Vec<u8> {
        if let Some(ref hash) = *self.hash.borrow() {
            return hash.to_vec();
        } else {
            let mut hasher = Sha3_256::default();

            hasher.update(b"\0");
            for (k, v) in &self.inner {
                hasher.update(b"key\x01");
                hasher.update(k);
                hasher.update(b"key\x02");
                hasher.update(v);
            }
            hasher.update(b"\x03");
            self.hash.replace(Some(hasher.finalize().to_vec()));
        }
        self.hash()
    }

    // Never anything to commit.
    fn commit(&mut self) -> Result<(), ManyError> {
        Ok(())
    }
}
