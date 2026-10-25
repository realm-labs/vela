use vela_common::Span;
use vela_def::{FunctionId, StateId};
use vela_mir::MirTypeContract;

use crate::ScriptFunctionHandle;

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StateStorage {
    Vm,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StateVisibility {
    Private,
    Public,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateDescriptor {
    pub id: StateId,
    pub qualified_name: String,
    pub visibility: StateVisibility,
    pub storage: StateStorage,
    pub type_contract: MirTypeContract,
    pub initializer: Option<FunctionId>,
    pub source_span: Option<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkedStateDescriptor {
    pub id: StateId,
    pub qualified_name: String,
    pub visibility: StateVisibility,
    pub storage: StateStorage,
    pub type_contract: MirTypeContract,
    pub initializer: Option<ScriptFunctionHandle>,
    pub source_span: Option<Span>,
}

#[cfg(test)]
impl StateDescriptor {
    pub(crate) fn test_vm(id: StateId, qualified_name: impl Into<String>) -> Self {
        Self {
            id,
            qualified_name: qualified_name.into(),
            visibility: StateVisibility::Private,
            storage: StateStorage::Vm,
            type_contract: MirTypeContract::Any,
            initializer: Some(FunctionId::new(1)),
            source_span: None,
        }
    }
}
