use crate::host::storage;
use many_error::ManyError;
use std::path::PathBuf;

pub struct ExecutionBackend {
    storage: storage::Storage,
}

impl ExecutionBackend {
    pub fn new(storage: PathBuf) -> Result<Self, ManyError> {
        let storage = storage::Storage::new(&storage)?;
        Ok(Self { storage })
    }
}
