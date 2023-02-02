use crate::config::StorageConfig;
use many_error::ManyError;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub trait KvStore: Send {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ManyError>;
    fn set(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), ManyError>;
    fn del(&mut self, key: &[u8]) -> Result<(), ManyError>;
    fn contains(&self, key: &[u8]) -> bool;
    fn size(&self, key: &[u8]) -> Option<usize>;
    fn hash(&self) -> Vec<u8>;
    fn commit(&mut self) -> Result<(), ManyError>;
}

pub struct NullKvStore;

impl KvStore for NullKvStore {
    fn get(&self, _key: &[u8]) -> Result<Option<Vec<u8>>, ManyError> {
        Ok(None)
    }

    fn set(&mut self, _key: Vec<u8>, _value: Vec<u8>) -> Result<(), ManyError> {
        Ok(())
    }

    fn del(&mut self, _key: &[u8]) -> Result<(), ManyError> {
        Ok(())
    }

    fn contains(&self, _key: &[u8]) -> bool {
        false
    }

    fn size(&self, _key: &[u8]) -> Option<usize> {
        None
    }

    fn hash(&self) -> Vec<u8> {
        Vec::new()
    }

    fn commit(&mut self) -> Result<(), ManyError> {
        Ok(())
    }
}

pub mod memory;
pub mod merk;

#[derive(Clone)]
pub struct StorageRef {
    inner: Arc<Mutex<dyn KvStore>>,
    prefix: Option<Vec<u8>>,
}

impl StorageRef {
    pub fn new(store: impl KvStore + 'static) -> Self {
        Self {
            inner: Arc::new(Mutex::new(store)),
            prefix: None,
        }
    }

    pub fn new_prefixed(store: impl KvStore + 'static, prefix: Vec<u8>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(store)),
            prefix: Some(prefix),
        }
    }

    pub fn cloned_root(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            prefix: None,
        }
    }

    pub fn cloned_prefixed(&self, prefix: Vec<u8>) -> Self {
        let mut new_prefix = self.prefix.clone().unwrap_or_default();
        new_prefix.extend(prefix);
        Self {
            inner: self.inner.clone(),
            prefix: Some(new_prefix),
        }
    }

    #[inline]
    fn _key<'a>(&self, key: Cow<'a, [u8]>) -> Cow<'a, [u8]> {
        match &self.prefix {
            None => key,
            Some(p) => {
                let mut new_key = Vec::with_capacity(key.len() + p.len());
                new_key.extend_from_slice(p.as_slice());
                new_key.extend_from_slice(key.as_ref());

                new_key.into()
            }
        }
    }
}

impl KvStore for StorageRef {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ManyError> {
        let key = self._key(key.into());
        self.inner
            .lock()
            .map_err(ManyError::unknown)?
            .get(key.as_ref())
    }

    fn set(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), ManyError> {
        let key = self._key(key.into());
        self.inner
            .lock()
            .map_err(ManyError::unknown)?
            .set(key.to_vec(), value)
    }

    fn del(&mut self, key: &[u8]) -> Result<(), ManyError> {
        let key = self._key(key.into());
        self.inner
            .lock()
            .map_err(ManyError::unknown)?
            .del(key.as_ref())
    }

    fn contains(&self, key: &[u8]) -> bool {
        self.inner.lock().unwrap().contains(key)
    }

    fn size(&self, key: &[u8]) -> Option<usize> {
        self.inner.lock().ok()?.size(key)
    }

    fn hash(&self) -> Vec<u8> {
        self.inner
            .lock()
            .map_err(ManyError::unknown)
            .unwrap()
            .hash()
    }

    fn commit(&mut self) -> Result<(), ManyError> {
        self.inner.lock().map_err(ManyError::unknown)?.commit()
    }
}

#[derive(Default)]
pub struct StorageLibrary {
    inner: BTreeMap<String, StorageRef>,
}

impl StorageLibrary {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn create(
        config: StorageConfig,
        root: impl AsRef<Path>,
        create_if_missing: bool,
    ) -> Result<Self, ManyError> {
        let mut storage = Self::new();

        for config in config {
            let (name, storage_ref) =
                config.create_ref(&mut storage, root.as_ref(), create_if_missing)?;
            if storage.inner.contains_key(&name) {
                return Err(ManyError::unknown("Storage name already exists."));
            }
            storage.inner.insert(name, storage_ref);
        }

        Ok(storage)
    }

    pub fn get(&self, name: impl AsRef<str>) -> Option<&StorageRef> {
        self.inner.get(name.as_ref())
    }

    pub fn get_mut(&mut self, name: impl AsRef<str>) -> Option<&mut StorageRef> {
        self.inner.get_mut(name.as_ref())
    }
}
