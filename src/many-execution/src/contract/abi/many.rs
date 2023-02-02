use super::{_read, _read_str, _store};
use crate::wasm::state::WasmContext;
use std::cmp::min;
use wasi_common::Error;
use wasmtime::Caller;

pub fn payload_size(caller: Caller<'_, WasmContext>) -> u32 {
    caller.data().payload_size().unwrap_or(0) as u32
}

pub fn payload_copy(mut caller: Caller<'_, WasmContext>, ptr: u32, len: u32) -> Result<u32, Error> {
    // TODO: remove clone here.
    let payload_bytes = caller.data().payload_bytes()?.to_vec();

    _store(&mut caller, ptr, len, |data| {
        data.copy_from_slice(&payload_bytes[..len as usize]);
        Ok(min(data.len(), payload_bytes.len()) as u32)
    })
}

pub fn sender_size(caller: Caller<'_, WasmContext>) -> Result<u32, Error> {
    Ok(caller.data().sender()?.to_vec().len() as u32)
}

pub fn sender_copy(mut caller: Caller<'_, WasmContext>, ptr: u32) -> Result<u32, Error> {
    let sender = caller.data().sender()?;
    let bytes = sender.to_vec();

    _store(&mut caller, ptr, bytes.len() as u32, |data| {
        data.copy_from_slice(&bytes);
        Ok(bytes.len() as u32)
    })
}

pub fn error_create(mut caller: Caller<'_, WasmContext>, code: i32) -> Result<u32, Error> {
    Ok(caller.data_mut().create_error(code).into())
}
pub fn error_message(
    mut caller: Caller<'_, WasmContext>,
    handle: u32,
    msg_ptr: u32,
    msg_len: u32,
) -> Result<(), Error> {
    let message = _read_str(&mut caller, msg_ptr, msg_len, |s| Ok(s.to_string()))?;
    let err = caller.data_mut().get_error_mut(handle.into())?;
    err.set_message(Some(message.to_string()));
    Ok(())
}
pub fn error_argument(
    mut caller: Caller<'_, WasmContext>,
    handle: u32,
    key_ptr: u32,
    key_len: u32,
    value_ptr: u32,
    value_len: u32,
) -> Result<(), Error> {
    let key = _read_str(&mut caller, key_ptr, key_len, |s| Ok(s.to_string()))?;
    let value = _read_str(&mut caller, value_ptr, value_len, |s| Ok(s.to_string()))?;
    let err = caller.data_mut().get_error_mut(handle.into())?;
    err.add_argument(key, value);
    Ok(())
}

pub fn return_error(mut caller: Caller<'_, WasmContext>, handle: u32) -> Result<(), Error> {
    let err = caller.data().get_error(handle.into())?.clone();
    caller.data_mut().set_return_value(Err(err))?;
    Ok(())
}

pub fn return_data(mut caller: Caller<'_, WasmContext>, ptr: u32, len: u32) -> Result<(), Error> {
    let data = _read(&mut caller, ptr, len, |data| Ok(Vec::from(data)))?;
    caller.data_mut().set_return_value(Ok(data))?;
    Ok(())
}
