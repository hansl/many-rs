use crate::wasm::state::WasmContext;
use std::any::Any;
use std::io;
use tracing::Level;
use wasi_common::file::FileType;
use wasi_common::{Error, ErrorExt, Table, WasiCtx, WasiFile};
use wasmtime::Linker;

/// Creates a logger as a WasiFile pipe.
struct TracingWasiFile {
    level: Level,
    buffer: String,
}

impl TracingWasiFile {
    pub fn new(level: Level) -> Self {
        Self {
            level,
            buffer: String::with_capacity(1024),
        }
    }
}

#[async_trait::async_trait]
impl WasiFile for TracingWasiFile {
    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn get_filetype(&mut self) -> Result<FileType, Error> {
        Ok(FileType::Pipe)
    }

    async fn write_vectored<'a>(&mut self, bufs: &[io::IoSlice<'a>]) -> Result<u64, Error> {
        let mut n = 0;
        for b in bufs {
            let v = String::from_utf8_lossy(b);

            if v.ends_with("\n") {
                match self.level {
                    Level::TRACE => tracing::trace!("{}{}", self.buffer, &v[..v.len() - 1]),
                    Level::DEBUG => tracing::debug!("{}{}", self.buffer, &v[..v.len() - 1]),
                    Level::INFO => tracing::info!("{}{}", self.buffer, &v[..v.len() - 1]),
                    Level::WARN => tracing::warn!("{}{}", self.buffer, &v[..v.len() - 1]),
                    Level::ERROR => tracing::error!("{}{}", self.buffer, &v[..v.len() - 1]),
                }
                self.buffer.clear();
            } else {
                self.buffer += v.as_ref();
            }
            n += v.len();
        }
        Ok(n.try_into().map_err(|c| Error::range().context(c))?)
    }
    async fn write_vectored_at<'a>(
        &mut self,
        _bufs: &[io::IoSlice<'a>],
        _offset: u64,
    ) -> Result<u64, Error> {
        Err(Error::seek_pipe())
    }
    async fn seek(&mut self, _pos: std::io::SeekFrom) -> Result<u64, Error> {
        Err(Error::seek_pipe())
    }
}

pub fn create_wasi_ctx() -> WasiCtx {
    let mut ctx = WasiCtx::new(
        wasmtime_wasi::random_ctx(),
        wasmtime_wasi::clocks::clocks_ctx(),
        wasmtime_wasi::sched_ctx(),
        Table::new(),
    );

    ctx.set_stdout(Box::new(TracingWasiFile::new(Level::INFO)));
    ctx.set_stderr(Box::new(TracingWasiFile::new(Level::WARN)));

    ctx
}

pub fn register_wasi(linker: &mut Linker<WasmContext>) -> Result<(), Error> {
    wasmtime_wasi::add_to_linker(linker, |s| s.wasi_ctx_mut())?;
    Ok(())
}
