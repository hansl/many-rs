use crate::storage::KvStore;
use many_error::ManyError;
use merk::Op;
use std::path::Path;

pub struct MerkStorage {
    merk: merk::Merk,
}

impl MerkStorage {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ManyError> {
        let mut opts = merk::Merk::default_db_opts();
        opts.create_if_missing(false);

        let merk = merk::Merk::open_opt(path, opts).map_err(ManyError::unknown)?;
        Ok(Self { merk })
    }
    pub fn new(path: impl AsRef<Path>, delete_if_exists: bool) -> Result<Self, ManyError> {
        if delete_if_exists && path.as_ref().exists() {
            std::fs::remove_dir_all(path.as_ref()).map_err(ManyError::unknown)?;
        }

        let merk = merk::Merk::open(path).map_err(ManyError::unknown)?;
        Ok(Self { merk })
    }
}

impl KvStore for MerkStorage {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ManyError> {
        self.merk.get(key).map_err(|e| ManyError::unknown(e))
    }

    fn set(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), ManyError> {
        self.merk
            .apply(&[(key, Op::Put(value))])
            .map_err(|e| ManyError::unknown(e))?;
        Ok(())
    }

    fn del(&mut self, key: &[u8]) -> Result<(), ManyError> {
        self.merk
            .apply(&[(key.to_vec(), Op::Delete)])
            .map_err(|e| ManyError::unknown(e))?;
        Ok(())
    }

    fn contains(&self, key: &[u8]) -> bool {
        self.merk.get(key).map_or(false, |v| v.is_some())
    }

    fn size(&self, key: &[u8]) -> Option<usize> {
        Some(self.merk.get(key).ok()??.len())
    }

    fn hash(&self) -> Vec<u8> {
        self.merk.root_hash().to_vec()
    }

    fn commit(&mut self) -> Result<(), ManyError> {
        self.merk.commit(&[]).map_err(|e| ManyError::unknown(e))
    }
}
