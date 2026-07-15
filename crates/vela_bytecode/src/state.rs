use vela_common::Span;
use vela_def::{FunctionId, StateId};
use vela_mir::MirTypeContract;

use crate::ScriptFunctionHandle;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StateStorage {
    Vm,
    Extern,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StateVisibility {
    Private,
    Public,
}

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

impl StateDescriptor {
    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    #[must_use]
    pub fn test_extern(id: StateId, qualified_name: impl Into<String>) -> Self {
        Self {
            id,
            qualified_name: qualified_name.into(),
            visibility: StateVisibility::Private,
            storage: StateStorage::Extern,
            type_contract: MirTypeContract::Any,
            initializer: None,
            source_span: None,
        }
    }
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
