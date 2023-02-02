use crate::wasm::state::WasmContext;
use wasi_common::Error;
use wasmtime::{Caller, Extern, Linker};

pub mod many;
pub mod store;
pub mod wasi_snapshot_preview1;

macro_rules! decl_many_imports {
    ( ($linker: ident, $mod: ident) => { $($name: ident),* $(,)? }) => {
        $(
        $linker
            .func_wrap(stringify!($mod), stringify!($name), $mod :: $name)
            .expect(concat!("Could not link fn ", stringify!($mod), " :: ", stringify!($name)));
        )*
    };
}

pub fn link(linker: &mut Linker<WasmContext>) -> Result<(), Error> {
    decl_many_imports!((linker, many) => {
        payload_size,
        payload_copy,
        sender_size,
        sender_copy,
        error_create,
        error_message,
        error_argument,
        return_error,
        return_data,
    });

    decl_many_imports!((linker, store) => {
        storage,
        get,
        set,
        size,
    });

    wasi_snapshot_preview1::register_wasi(linker)?;

    Ok(())
}

// Utility functions for this module and its sub-modules.

pub(self) fn _read<R>(
    caller: &mut Caller<'_, WasmContext>,
    ptr: u32,
    len: u32,
    func: impl FnOnce(&[u8]) -> Result<R, Error>,
) -> Result<R, Error> {
    let memory = match caller.get_export("memory") {
        Some(Extern::Memory(mem)) => mem,
        _ => return Err(Error::msg("failed to find client memory")),
    };
    let data = memory
        .data(&caller)
        .get(ptr as usize..)
        .and_then(|arr| arr.get(..len as usize))
        .ok_or_else(|| Error::msg("pointer/length out of bounds"))?;

    func(data)
}

pub(self) fn _read_str<R>(
    caller: &mut Caller<'_, WasmContext>,
    ptr: u32,
    len: u32,
    func: impl FnOnce(&str) -> Result<R, Error>,
) -> Result<R, Error> {
    _read(caller, ptr, len, |data| {
        let s = std::str::from_utf8(data).map_err(|_| Error::msg("invalid utf-8 string"))?;
        func(s)
    })
}

pub(self) fn _store<T>(
    caller: &mut Caller<WasmContext>,
    ptr: u32,
    len: u32,
    func: impl FnOnce(&mut [u8]) -> Result<T, Error>,
) -> Result<T, Error> {
    let memory = match caller.get_export("memory") {
        Some(Extern::Memory(mem)) => mem,
        _ => return Err(Error::msg("failed to find host memory")),
    };

    let data = memory
        .data_mut(caller)
        .get_mut(ptr as usize..)
        .and_then(|arr| arr.get_mut(..len as usize))
        .ok_or_else(|| Error::msg("pointer/length out of bounds"))?;
    func(data)
}
