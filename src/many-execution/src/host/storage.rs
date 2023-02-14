use many_error::ManyError;
use many_identity::Address;
use many_types::cbor_type_decl;
use merk::{Merk, Op};
use minicbor::{Decode, Encode};
use std::io::Read;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

fn key_for_module_info(address: Address) -> Vec<u8> {
    [b"module_info/".to_vec(), address.to_vec()].concat()
}

cbor_type_decl!(
    pub struct ModuleInfo {
        0 => version: u64,
        1 => address: Address,
        2 => module: PathBuf,
        3 => memory: PathBuf,
    }
);

impl ModuleInfo {
    pub fn new(address: Address, module: PathBuf, memory: PathBuf) -> Self {
        Self {
            version: 0,
            address,
            module,
            memory,
        }
    }
}

pub struct StorageRef<Inner>
where
    Inner: Encode<()> + for<'a> Decode<'a, ()>,
{
    merk: Arc<RwLock<Merk>>,
    key: Vec<u8>,
    dirty: AtomicBool,
    inner: Inner,
}

impl<Inner> StorageRef<Inner>
where
    Inner: Encode<()> + for<'a> Decode<'a, ()>,
{
    pub fn new(merk: Arc<RwLock<Merk>>, key: Vec<u8>, inner: Inner) -> Result<Self, ManyError> {
        Ok(Self {
            merk,
            key,
            dirty: AtomicBool::new(true),
            inner,
        })
    }

    pub fn load(merk: Arc<RwLock<Merk>>, key: Vec<u8>) -> Result<Option<Self>, ManyError> {
        let bytes = {
            let m = merk.read().map_err(ManyError::unknown)?;
            m.get(&key).map_err(ManyError::unknown)
        }?;
        if let Some(bytes) = bytes {
            let inner = minicbor::decode(&bytes).map_err(ManyError::unknown)?;

            Ok(Some(Self {
                merk,
                key,
                dirty: AtomicBool::new(false),
                inner,
            }))
        } else {
            Ok(None)
        }
    }
}

impl<Inner> Deref for StorageRef<Inner>
where
    Inner: Encode<()> + for<'a> Decode<'a, ()>,
{
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<Inner> DerefMut for StorageRef<Inner>
where
    Inner: Encode<()> + for<'a> Decode<'a, ()>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.dirty.store(true, Ordering::Relaxed);
        &mut self.inner
    }
}

impl<Inner> Drop for StorageRef<Inner>
where
    Inner: Encode<()> + for<'a> Decode<'a, ()>,
{
    fn drop(&mut self) {
        let Self {
            dirty,
            inner,
            merk,
            key,
        } = self;
        let key: Vec<u8> = key.drain(..).collect();

        if dirty.load(Ordering::Relaxed) {
            if let Ok(e) = minicbor::to_vec(&inner) {
                if let Ok(mut m) = merk.write() {
                    let _ = m.apply(&[(key, Op::Put(e))]);
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

    pub fn module_info(&mut self, address: Address) -> Result<StorageRef<ModuleInfo>, ManyError> {
        StorageRef::load(Arc::clone(&self.merk), key_for_module_info(address))?
            .ok_or_else(|| ManyError::unknown("Unknown module."))
    }

    pub fn new_module_info(
        &mut self,
        info: ModuleInfo,
    ) -> Result<StorageRef<ModuleInfo>, ManyError> {
        StorageRef::new(
            Arc::clone(&self.merk),
            key_for_module_info(info.address),
            info,
        )
    }
}
