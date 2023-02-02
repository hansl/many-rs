use super::state;
use crate::config::ModuleConfig;
use crate::contract::abi;
use crate::storage::StorageLibrary;
use crate::wasm::state::CallContext;
use abi::wasi_snapshot_preview1::create_wasi_ctx;
use anyhow::anyhow;
use many_error::ManyError;
use many_protocol::RequestMessage;
use state::WasmContext;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::debug;
use wasmtime::{Engine, Linker, Module, Store};

#[derive(Default)]
struct ModuleLibrary {
    endpoints: BTreeMap<String, usize>,
    names: BTreeMap<String, usize>,
    modules: Vec<Arc<Module>>,
}

impl ModuleLibrary {
    pub fn add(&mut self, module: Module, name: Cow<str>) -> Result<(), anyhow::Error> {
        let endpoints = module
            .exports()
            .into_iter()
            .filter(|e| e.ty().func().is_some() && e.name().starts_with("endpoint "))
            .map(|e| e.name()[9..].to_string())
            .collect::<Vec<String>>();

        debug!("Adding module: endpoints = {endpoints:?}");

        for ep in endpoints.iter() {
            if self.endpoints.contains_key(ep) {
                return Err(anyhow!("Endpoint {ep} already registered."));
            }
        }

        let idx = self.modules.len();
        self.modules.push(Arc::new(module));
        for ep in endpoints {
            self.endpoints.insert(ep, idx);
        }
        self.names.insert(name.into_owned(), idx);

        Ok(())
    }

    pub fn by_endpoint(&self, endpoint: &str) -> Option<Arc<Module>> {
        let idx = self.endpoints.get(endpoint)?;
        self.modules.get(*idx).map(Arc::clone)
    }

    pub fn by_name(&self, name: &str) -> Option<Arc<Module>> {
        let idx = self.names.get(name)?;
        self.modules.get(*idx).map(Arc::clone)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<Module>> {
        self.modules.iter()
    }
}

pub struct WasmEngine {
    store: Store<WasmContext>,
    linker: Linker<WasmContext>,
    modules: ModuleLibrary,
}

impl WasmEngine {
    pub fn new(storage: StorageLibrary) -> Result<Self, anyhow::Error> {
        let engine = Engine::default();
        let store = Store::new(&engine, WasmContext::new(storage, create_wasi_ctx()));
        let mut linker = Linker::new(store.engine());
        abi::link(&mut linker)?;

        Ok(Self {
            store,
            linker,
            modules: ModuleLibrary::default(),
        })
    }

    pub fn add_module_config(&mut self, config: ModuleConfig) -> Result<(), anyhow::Error> {
        for ref config in config {
            let module: Module = Module::from_file(self.store.engine(), &config.path)
                .map_err(|e| anyhow!("{}", e))?;

            // Instantiate at least once to optimize.
            self.linker.instantiate(&mut self.store, &module)?;

            self.modules.add(module, config.name())?;
        }

        Ok(())
    }

    pub fn init(&mut self, init: ModuleConfig) -> Result<(), anyhow::Error> {
        // First, initialize with the init modules.
        for ref config in init {
            let module: Module = Module::from_file(self.store.engine(), &config.path)
                .map_err(|e| anyhow!("{}", e))?;

            // Instantiate it at least once.
            self.linker.instantiate(&mut self.store, &module)?;

            let payload = config.arg.to_string();
            self.store
                .data_mut()
                .set_call_context(CallContext::Initialize(payload.into_bytes()));

            let _: () = self.call_contract_method(&module, "init", ())?;
        }

        Ok(())
    }

    pub fn call_endpoint(&mut self, message: &RequestMessage) -> Result<Vec<u8>, ManyError> {
        let endpoint = message.method.to_string();
        let module = self
            .modules
            .by_endpoint(&endpoint)
            .ok_or_else(|| ManyError::unknown("Endpoint not found"))?;

        self.store
            .data_mut()
            .set_call_context(CallContext::ManyRequest(message.clone(), None));

        let _: () = self.call_contract_method(&module, &format!("endpoint {}", endpoint), ())?;

        let result = match self.store.data_mut().response() {
            Ok(x) => x,
            Err(t) => Err(ManyError::unknown(format!("trapped: {}", t.to_string()))),
        };
        self.store.data_mut().reset();
        result
    }

    fn call_contract_method<Params, Results>(
        &mut self,
        module: &Module,
        name: &str,
        args: Params,
    ) -> Result<Results, ManyError>
    where
        Params: wasmtime::WasmParams,
        Results: wasmtime::WasmResults,
    {
        let instance = self
            .linker
            .instantiate(&mut self.store, module)
            .expect("Could not instantiate");

        let func = instance
            .get_typed_func::<Params, Results, _>(&mut self.store, name)
            .map_err(|e| ManyError::unknown(e))?;

        func.call(&mut self.store, args)
            .map_err(|e| ManyError::unknown(e))
    }
}
