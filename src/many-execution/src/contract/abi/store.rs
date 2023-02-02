use super::{_read, _read_str, _store};
use crate::storage::KvStore;
use crate::wasm::state::WasmContext;
use anyhow::anyhow;
use wasi_common::Error;
use wasmtime::Caller;

pub fn storage(
    mut caller: Caller<'_, WasmContext>,
    name_ptr: u32,
    name_len: u32,
) -> Result<u32, Error> {
    let name = _read_str(&mut caller, name_ptr, name_len, |name| Ok(name.to_owned()))?;
    caller.data_mut().create_storage(&name).map(|x| x.into())
}

pub fn size(
    mut caller: Caller<'_, WasmContext>,
    handle: u32,
    key_ptr: u32,
    key_len: u32,
) -> Result<u32, Error> {
    let key = _read(&mut caller, key_ptr, key_len, |key| Ok(key.to_owned()))?;

    let storage_ref = caller.data().get_storage(handle.into())?;
    Ok(storage_ref.size(&key).unwrap_or(0) as u32)
}

pub fn get(
    mut caller: Caller<'_, WasmContext>,
    handle: u32,
    key_ptr: u32,
    key_len: u32,
    buffer_ptr: u32,
    buffer_len: u32,
) -> Result<u32, Error> {
    let key = _read(&mut caller, key_ptr, key_len, |key| Ok(key.to_owned()))?;

    let storage_ref = caller.data().get_storage(handle.into())?;
    let value = storage_ref.get(&key)?;
    if let Some(value) = value {
        let len = value.len();

        _store(&mut caller, buffer_ptr, buffer_len, |buffer| {
            buffer.copy_from_slice(&value.as_slice()[0..buffer_len as usize]);
            Ok(())
        })?;

        Ok(len as u32)
    } else {
        Ok(0)
    }
}

pub fn set(
    mut caller: Caller<'_, WasmContext>,
    handle: u32,
    key_ptr: u32,
    key_len: u32,
    value_ptr: u32,
    value_len: u32,
) -> Result<(), Error> {
    let key = _read(&mut caller, key_ptr, key_len, |key| Ok(key.to_owned()))?;
    let value = _read(&mut caller, value_ptr, value_len, |value| {
        Ok(value.to_owned())
    })?;

    let storage_ref = caller.data_mut().get_storage_mut(handle.into())?;
    storage_ref.set(key, value).map_err(|e| anyhow!("{e}"))
}
