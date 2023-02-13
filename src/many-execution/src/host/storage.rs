use many_error::ManyError;
use many_identity::Address;
use many_types::cbor_type_decl;
use merk::{Merk, Op};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

fn key_for_module_info(address: Address) -> Vec<u8> {
    [b"module_info/".to_vec(), address.to_vec()].concat()
}

cbor_type_decl!(
    pub struct ModuleInfo {
        0 => address: Address,
        1 => module: PathBuf,
        2 => memory: PathBuf,
    }
);

pub struct ModuleInfoRef {
    merk: Arc<RwLock<Merk>>,
    dirty: AtomicBool,
    inner: ModuleInfo,
}

impl ModuleInfoRef {
    pub fn load(merk: Arc<RwLock<Merk>>, address: Address) -> Result<Self, ManyError> {
        let bytes = {
            let m = merk.read().map_err(ManyError::unknown)?;
            m.get(&key_for_module_info(address))
                .map_err(ManyError::unknown)
        }?;
        if let Some(bytes) = bytes {
            let inner = minicbor::decode(&bytes).map_err(ManyError::unknown)?;

            Ok(Self {
                merk,
                dirty: AtomicBool::new(false),
                inner,
            })
        } else {
            Err(ManyError::unknown("Module not found."))
        }
    }
}

impl Deref for ModuleInfoRef {
    type Target = ModuleInfo;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ModuleInfoRef {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.dirty.store(true, Ordering::Relaxed);
        &mut self.inner
    }
}

impl Drop for ModuleInfoRef {
    fn drop(&mut self) {
        if self.dirty.load(Ordering::Relaxed) {
            if let Ok(e) = minicbor::to_vec(&self.inner) {
                if let Ok(mut m) = self.merk.write() {
                    let _ = m.apply(&[(key_for_module_info(self.inner.address), Op::Put(e))]);
                }
            }
        }
    }
}

pub struct Storage {
    merk: Arc<RwLock<Merk>>,
}

impl Storage {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, ManyError> {
        let merk = Merk::open(path).map_err(ManyError::unknown)?;

        Ok(Self {
            merk: Arc::new(RwLock::new(merk)),
        })
    }

    pub fn module_info(&mut self, address: Address) -> Result<ModuleInfoRef, ManyError> {
        ModuleInfoRef::load(Arc::clone(&self.merk), address)
    }
}
