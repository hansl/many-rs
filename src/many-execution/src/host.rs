//! The host is the API to create, install and manage smart contracts on this
//! neighborhood.
use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;
use many_types::cbor_type_decl;
use minicbor::bytes::ByteVec;

pub mod backend;
pub mod storage;

cbor_type_decl!(
    pub struct ListArgs {}

    pub struct ListReturns {
        0 => list: Vec<Address>,
    }

    pub struct CreateArgs {
    }

    pub struct CreateReturns {
        0 => address: Address,
    }

    pub struct InstallArgs {
        0 => address: Address,
        1 => wasm: ByteVec,
    }

    pub struct InstallReturns {}
);

#[many_module(name = ExecutionModule, id = 1100, namespace = execution)]
pub trait ExecutionModuleBackend: Send {
    fn list(&self, args: ListArgs) -> Result<ListReturns, ManyError>;
    fn create(&self, args: CreateArgs) -> Result<CreateReturns, ManyError>;
    fn install(&self, args: InstallArgs) -> Result<InstallReturns, ManyError>;
}
