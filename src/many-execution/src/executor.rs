use crate::wasm_engine::WasmEngine;
use async_trait::async_trait;
use coset::CoseSign1;
use many_identity::verifiers::AnonymousVerifier;
use many_identity::Identity;
use many_identity_dsa::CoseKeyVerifier;
use many_protocol::{
    decode_request_from_cose_sign1, encode_cose_sign1_from_response, ResponseMessage,
};
use many_server::transport::LowLevelManyRequestHandler;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, RwLock};

pub struct WasmExecutor<I: Identity> {
    engine: RwLock<WasmEngine>,
    identity: Arc<I>,
}

impl<I: Identity> WasmExecutor<I> {
    pub fn new(engine: WasmEngine, identity: I) -> Self {
        Self {
            engine: RwLock::new(engine),
            identity: Arc::new(identity),
        }
    }
}

impl<I: Identity> Debug for WasmExecutor<I> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmExecutor")
            .field("identity", &self.identity.address())
            .finish()
    }
}

#[async_trait]
impl<'a, I: Identity + std::marker::Sync> LowLevelManyRequestHandler for WasmExecutor<I> {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String> {
        let request =
            decode_request_from_cose_sign1(&envelope, &(AnonymousVerifier, CoseKeyVerifier))
                .map_err(|e| e.to_string())?;

        let mut engine = self
            .engine
            .write()
            .map_err(|_| String::from("Lock is poisoned."))?;

        let data = engine.call_endpoint(&request);

        let response = ResponseMessage::from_request(&request, &self.identity.address(), data);
        encode_cose_sign1_from_response(response, &self.identity).map_err(|e| e.to_string())
    }
}
