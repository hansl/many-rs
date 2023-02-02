use crate::storage::{StorageLibrary, StorageRef};
use anyhow::Error;
use many_error::ManyError;
use many_identity::Address;
use many_protocol::RequestMessage;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::SystemTime;
use wasi_common::WasiCtx;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(transparent)]
pub struct RegistryHandle(u32);

static UNIQUE_ID: AtomicU32 = AtomicU32::new(0);

impl RegistryHandle {
    pub fn new() -> Self {
        Self(UNIQUE_ID.fetch_add(1, Ordering::SeqCst))
    }

    pub fn null() -> Self {
        Self(u32::MAX)
    }

    pub fn is_null(&self) -> bool {
        self.0 == u32::MAX
    }

    pub fn usize(&self) -> usize {
        self.0 as usize
    }
}

impl Into<RegistryHandle> for u32 {
    fn into(self) -> RegistryHandle {
        RegistryHandle(self)
    }
}

impl Into<u32> for RegistryHandle {
    fn into(self) -> u32 {
        self.0
    }
}

impl Into<RegistryHandle> for usize {
    fn into(self) -> RegistryHandle {
        RegistryHandle(self as u32)
    }
}

impl Into<usize> for RegistryHandle {
    fn into(self) -> usize {
        self.0 as usize
    }
}

#[non_exhaustive]
pub enum RegistryObject {
    Error(ManyError),
    Storage(StorageRef),
}

impl RegistryObject {
    pub fn as_error(&self) -> Option<&ManyError> {
        match self {
            RegistryObject::Error(e) => Some(e),
            _ => None,
        }
    }
    pub fn as_error_mut(&mut self) -> Option<&mut ManyError> {
        match self {
            RegistryObject::Error(e) => Some(e),
            _ => None,
        }
    }
    pub fn as_storage(&self) -> Option<&StorageRef> {
        match self {
            RegistryObject::Storage(sref) => Some(sref),
            _ => None,
        }
    }
    pub fn as_storage_mut(&mut self) -> Option<&mut StorageRef> {
        match self {
            RegistryObject::Storage(sref) => Some(sref),
            _ => None,
        }
    }
}

#[derive(Default)]
struct HandleRegistry {
    inner: BTreeMap<RegistryHandle, RegistryObject>,
}

impl HandleRegistry {
    fn create(&mut self, object: RegistryObject) -> RegistryHandle {
        let handle = RegistryHandle::new();
        self.inner.insert(handle, object);
        handle
    }

    pub fn get_error(&self, handle: RegistryHandle) -> Option<&ManyError> {
        self.inner.get(&handle)?.as_error()
    }

    pub fn get_error_mut(&mut self, handle: RegistryHandle) -> Option<&mut ManyError> {
        self.inner.get_mut(&handle)?.as_error_mut()
    }

    pub fn error(&mut self, code: i32) -> RegistryHandle {
        self.create(RegistryObject::Error(ManyError::new(
            (code as i64).try_into().unwrap(),
            None,
            BTreeMap::new(),
        )))
    }

    pub fn create_storage(&mut self, storage_ref: StorageRef) -> RegistryHandle {
        self.create(RegistryObject::Storage(storage_ref))
    }

    pub fn get_storage(&self, handle: RegistryHandle) -> Option<&StorageRef> {
        self.inner.get(&handle)?.as_storage()
    }

    pub fn get_storage_mut(&mut self, handle: RegistryHandle) -> Option<&mut StorageRef> {
        self.inner.get_mut(&handle)?.as_storage_mut()
    }
}

pub enum CallContext {
    None,
    Initialize(Vec<u8>),
    ManyRequest(RequestMessage, Option<Result<Vec<u8>, ManyError>>),
}

impl CallContext {
    pub fn as_initialize(&self) -> Option<&Vec<u8>> {
        match self {
            Self::Initialize(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_initialize_mut(&mut self) -> Option<&mut Vec<u8>> {
        match self {
            Self::Initialize(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_many_request(
        &self,
    ) -> Option<(&RequestMessage, &Option<Result<Vec<u8>, ManyError>>)> {
        match self {
            Self::ManyRequest(req, res) => Some((req, res)),
            _ => None,
        }
    }

    pub fn as_many_request_mut(
        &mut self,
    ) -> Option<(&mut RequestMessage, &mut Option<Result<Vec<u8>, ManyError>>)> {
        match self {
            Self::ManyRequest(req, res) => Some((req, res)),
            _ => None,
        }
    }
}

pub struct WasmContext {
    call_context: CallContext,
    registry: HandleRegistry,

    storage_library: StorageLibrary,
    storage_cache: BTreeMap<String, RegistryHandle>,
    wasi_ctx: WasiCtx,
}

impl WasmContext {
    pub fn new(storage_library: StorageLibrary, wasi_ctx: WasiCtx) -> Self {
        Self {
            call_context: CallContext::None,
            registry: Default::default(),
            storage_library,
            storage_cache: BTreeMap::new(),
            wasi_ctx,
        }
    }

    pub fn reset(&mut self) {
        self.call_context = CallContext::None;
    }

    pub fn response(&mut self) -> Result<Result<Vec<u8>, ManyError>, Error> {
        match &mut self.call_context {
            CallContext::ManyRequest(_, response) => response
                .take()
                .ok_or_else(|| Error::msg("No return value was set")),
            _ => Err(Error::msg("Invalid context")),
        }
    }

    pub fn wasi_ctx(&self) -> &WasiCtx {
        &self.wasi_ctx
    }

    pub fn wasi_ctx_mut(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }

    pub fn set_call_context(&mut self, ctx: CallContext) {
        self.call_context = ctx;
    }

    pub fn set_return_value(&mut self, value: Result<Vec<u8>, ManyError>) -> Result<(), Error> {
        match &mut self.call_context {
            CallContext::ManyRequest(_, response) => match response.replace(value) {
                None => Ok(()),
                Some(_) => Err(Error::msg("return state already set")),
            },
            _ => Err(Error::msg("Invalid call context")),
        }
    }

    pub fn arg(&self) -> Option<&Vec<u8>> {
        self.call_context.as_initialize()
    }

    pub fn request(&self) -> Result<&RequestMessage, Error> {
        self.call_context
            .as_many_request()
            .map(|(req, _)| req)
            .ok_or_else(|| Error::msg("Invalid context"))
    }

    pub fn payload_size(&self) -> Result<usize, Error> {
        Ok(self.request()?.data.len())
    }
    pub fn payload_bytes(&self) -> Result<&[u8], Error> {
        Ok(self.request()?.data.as_slice())
    }

    pub fn sender(&self) -> Result<Address, Error> {
        Ok(self.request()?.from.unwrap_or_default())
    }
    pub fn dest(&self) -> Result<Address, Error> {
        Ok(self.request()?.to)
    }

    pub fn get_error(&self, handle: RegistryHandle) -> Result<&ManyError, Error> {
        self.registry
            .get_error(handle)
            .ok_or_else(|| Error::msg("invalid handle"))
    }
    pub fn get_error_mut(&mut self, handle: RegistryHandle) -> Result<&mut ManyError, Error> {
        self.registry
            .get_error_mut(handle)
            .ok_or_else(|| Error::msg("invalid handle"))
    }
    pub fn create_error(&mut self, code: i32) -> RegistryHandle {
        self.registry.error(code)
    }

    pub fn create_storage(&mut self, name: &str) -> Result<RegistryHandle, Error> {
        // Make sure it hasn't already been created.
        Ok(*self.storage_cache.entry(name.to_string()).or_insert(
            self.registry.create_storage(
                self.storage_library
                    .get(name)
                    .ok_or_else(|| Error::msg("Unknown storage name."))?
                    .clone(),
            ),
        ))
    }

    pub fn get_storage(&self, handle: u32) -> Result<&StorageRef, Error> {
        self.registry
            .get_storage(handle.into())
            .ok_or_else(|| Error::msg("Unknown handle or not a storage."))
    }

    pub fn get_storage_mut(&mut self, handle: u32) -> Result<&mut StorageRef, Error> {
        self.registry
            .get_storage_mut(handle.into())
            .ok_or_else(|| Error::msg("Unknown handle or not a storage."))
    }

    pub fn get_time(&self) -> SystemTime {
        SystemTime::now()
    }
}
