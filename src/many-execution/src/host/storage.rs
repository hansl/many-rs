use many_error::ManyError;
use many_identity::Address;
use many_types::cbor_type_decl;
use merk::{Merk, Op};
use minicbor::{Decode, Encode};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn key_for_module_info(address: Address) -> Vec<u8> {
    [b"module_info/".to_vec(), address.to_vec()].concat()
}

cbor_type_decl!(
    struct ModuleInner {
        0 => address: Address,
        1 => module: PathBuf,
        2 => memory: PathBuf,
    }
);

pub struct ModuleInfo {
    merk: Arc<Merk>,
    dirty:
    pub inner: ModuleInner,
}

impl ModuleInfo {
    pub fn new(merk: Arc<Merk>, address: Address) -> Result<Self, ManyError> {
        let bytes = merk
            .get(&key_for_module_info(address))
            .map_err(ManyError::unknown)?;
        if let Some(bytes) = bytes {
            let inner = minicbor::decode(&bytes).map_err(ManyError::unknown)?;

            Ok(Self { merk, inner })
        } else {
            Err(ManyError::unknown("Module not found."))
        }
    }
}
impl Drop for ModuleInfo {
    fn drop(&mut self) {
        if let Ok(e) = minicbor::to_vec(&self.inner) {
            let _ = self
                .merk
                .apply(&[(key_for_module_info(self.inner.address), Op::Put(e))]);
        }
    }
}

pub struct Storage {
    merk: Arc<Merk>,
}

impl Storage {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, ManyError> {
        let merk = Merk::open(path).map_err(ManyError::unknown)?;

        Ok(Self {
            merk: Arc::new(merk),
        })
    }

    pub fn module_info(&mut self, address: Address) -> Result<ModuleInfo, ManyError> {
        ModuleInfo::new(Arc::clone(&self.merk), address)
    }
}
