use std::sync::Arc;

use vela_common::HostMethodId;
use vela_host::HostPath;
use vela_reflect::TypeKey;
use vela_vm::{HostExecution, Value, VmResult};

use crate::{EffectSet, FunctionAccess, TypeHint};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeMethodDesc {
    pub owner: TypeKey,
    pub id: HostMethodId,
    pub name: String,
    pub params: Vec<NativeMethodParamDesc>,
    pub returns: TypeHint,
    pub effects: EffectSet,
    pub access: FunctionAccess,
    pub docs: Option<String>,
}

impl NativeMethodDesc {
    #[must_use]
    pub fn new(owner: TypeKey, id: HostMethodId, name: impl Into<String>) -> Self {
        Self {
            owner,
            id,
            name: name.into(),
            params: Vec::new(),
            returns: TypeHint::Any,
            effects: EffectSet::default(),
            access: FunctionAccess::default(),
            docs: None,
        }
    }

    #[must_use]
    pub fn param(mut self, name: impl Into<String>, hint: TypeHint) -> Self {
        self.params.push(NativeMethodParamDesc {
            name: name.into(),
            hint,
        });
        self
    }

    #[must_use]
    pub fn returns(mut self, hint: TypeHint) -> Self {
        self.returns = hint;
        self
    }

    #[must_use]
    pub fn effects(mut self, effects: EffectSet) -> Self {
        self.effects = effects;
        self
    }

    #[must_use]
    pub fn access(mut self, access: FunctionAccess) -> Self {
        self.access = access;
        self
    }

    #[must_use]
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeMethodParamDesc {
    pub name: String,
    pub hint: TypeHint,
}

pub type NativeMethodFunction = Arc<
    dyn for<'host> Fn(&HostPath, &[Value], &mut HostExecution<'host>) -> VmResult<Value>
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone)]
pub struct NativeMethodEntry {
    pub desc: NativeMethodDesc,
    pub function: NativeMethodFunction,
}

impl NativeMethodEntry {
    #[must_use]
    pub fn new(
        desc: NativeMethodDesc,
        function: impl for<'host> Fn(&HostPath, &[Value], &mut HostExecution<'host>) -> VmResult<Value>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            desc,
            function: Arc::new(function),
        }
    }
}
