use crate::storage::{memory, merk, NullKvStore, StorageLibrary, StorageRef};
use anyhow::anyhow;
use either::Either;
use many_error::ManyError;
use serde::Deserializer;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tracing::debug;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SingleStorageConfig {
    Memory {
        name: String,
        values: Option<BTreeMap<String, String>>,
    },
    Merk {
        name: String,
        path: PathBuf,
    },
    Prefixed {
        name: String,
        prefix: String,
        backend: String,
    },
    Null {
        name: String,
    },
}

impl SingleStorageConfig {
    pub fn create_ref(
        self,
        storage: &mut StorageLibrary,
        root: impl AsRef<Path>,
        create_if_missing: bool,
    ) -> Result<(String, StorageRef), ManyError> {
        match self {
            SingleStorageConfig::Memory { name, values } => {
                debug!(r#type = "memory", name);
                let mut memory = memory::MemoryStorage::default();
                if let Some(values) = values {
                    for (key, value) in values {
                        let key = hex::decode(key).map_err(ManyError::unknown)?;
                        let value = hex::decode(value).map_err(ManyError::unknown)?;
                        memory.insert(key, value);
                    }
                }
                Ok((name, StorageRef::new(memory)))
            }
            SingleStorageConfig::Merk { name, path } => {
                let path = root.as_ref().join(path);
                debug!(
                    r#type = "merk",
                    name,
                    path = path.to_string_lossy().as_ref(),
                    create_if_missing
                );
                let merk = if create_if_missing {
                    merk::MerkStorage::new(&path, true)?
                } else {
                    merk::MerkStorage::load(&path)?
                };
                Ok((name, StorageRef::new(merk)))
            }
            SingleStorageConfig::Prefixed {
                name,
                prefix,
                backend,
            } => {
                let backend = storage
                    .get_mut(backend)
                    .ok_or_else(|| ManyError::unknown("Unknown backend storage."))?;

                Ok((name, backend.cloned_prefixed(prefix.into_bytes())))
            }
            SingleStorageConfig::Null { name } => Ok((name, StorageRef::new(NullKvStore))),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct StorageConfig(Vec<SingleStorageConfig>);

impl IntoIterator for StorageConfig {
    type Item = SingleStorageConfig;
    type IntoIter = std::vec::IntoIter<SingleStorageConfig>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

thread_local! {
    static CURRENT_PATH: RefCell<PathBuf> = RefCell::new(PathBuf::new());
}

fn maybe_load<'de, D, T: serde::de::DeserializeOwned>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
{
    let v: Either<PathBuf, T> = either::serde_untagged::deserialize(deserializer)?;
    match v {
        Either::Left(path) => CURRENT_PATH.with(|p| {
            let content = std::fs::read_to_string(p.borrow().join(&path))
                .map_err(|_e| serde::de::Error::custom("Could not open file"))?;
            json5::from_str(&content).map_err(|_| serde::de::Error::custom("Could not parse file"))
        }),
        Either::Right(t) => Ok(t),
    }
}

fn prefix_root<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let pb: PathBuf = serde::Deserialize::deserialize(deserializer)?;
    CURRENT_PATH.with(|j| Ok(j.borrow().join(&pb)))
}

#[derive(Serialize, Deserialize)]
pub struct SingleModuleConfig {
    pub name: Option<String>,

    #[serde(deserialize_with = "prefix_root")]
    pub path: PathBuf,

    #[serde(deserialize_with = "maybe_load")]
    pub arg: Value,
}

impl SingleModuleConfig {
    pub fn name(&self) -> Cow<'_, str> {
        self.name
            .as_ref()
            .map(|n| Cow::Borrowed(n.as_str()))
            .or_else(|| self.path.file_name().map(|x| x.to_string_lossy()))
            .unwrap_or_default()
    }
}

#[derive(Serialize, Deserialize)]
#[repr(transparent)]
pub struct ModuleConfig(Vec<SingleModuleConfig>);

impl IntoIterator for ModuleConfig {
    type Item = SingleModuleConfig;
    type IntoIter = std::vec::IntoIter<SingleModuleConfig>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Serialize, Deserialize)]
pub struct WasmConfig {
    pub init: ModuleConfig,
    pub modules: ModuleConfig,
    pub storages: StorageConfig,
}

impl WasmConfig {
    pub fn load(path: &Path) -> Result<Self, anyhow::Error> {
        CURRENT_PATH.with(|p| {
            *p.borrow_mut() = path.parent().map(Path::to_path_buf).unwrap_or_default();
        });

        json5::from_str(&std::fs::read_to_string(path)?)
            .map_err(|e| anyhow!("Could not parse module config: {e}"))
    }
}
