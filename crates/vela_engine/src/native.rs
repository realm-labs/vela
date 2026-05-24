use std::sync::Arc;

use vela_common::FunctionId;
use vela_reflect::TypeKey;
use vela_vm::{HostExecution, Value, VmResult};

pub type NativeFunctionId = FunctionId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeFunctionDesc {
    pub id: NativeFunctionId,
    pub name: String,
    pub params: Vec<NativeParamDesc>,
    pub returns: TypeHint,
    pub effects: EffectSet,
    pub access: FunctionAccess,
    pub docs: Option<String>,
}

impl NativeFunctionDesc {
    #[must_use]
    pub fn new(name: impl Into<String>, id: NativeFunctionId) -> Self {
        Self {
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
        self.params.push(NativeParamDesc {
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
pub struct NativeParamDesc {
    pub name: String,
    pub hint: TypeHint,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectSet {
    pub reads_host: bool,
    pub writes_host: bool,
    pub emits_events: bool,
}

impl EffectSet {
    #[must_use]
    pub const fn pure() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: false,
        }
    }

    #[must_use]
    pub const fn host_read() -> Self {
        Self {
            reads_host: true,
            writes_host: false,
            emits_events: false,
        }
    }

    #[must_use]
    pub const fn host_write() -> Self {
        Self {
            reads_host: true,
            writes_host: true,
            emits_events: false,
        }
    }

    #[must_use]
    pub const fn event_emit() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionAccess {
    pub public: bool,
    pub reflect_callable: bool,
    pub required_permissions: Vec<String>,
}

impl FunctionAccess {
    #[must_use]
    pub fn public() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn reflect_callable(mut self, reflect_callable: bool) -> Self {
        self.reflect_callable = reflect_callable;
        self
    }

    #[must_use]
    pub fn require_permission(mut self, permission: impl Into<String>) -> Self {
        self.required_permissions.push(permission.into());
        self
    }
}

impl Default for FunctionAccess {
    fn default() -> Self {
        Self {
            public: true,
            reflect_callable: false,
            required_permissions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeHint {
    Any,
    Null,
    Bool,
    Int,
    Float,
    String,
    Array,
    Map,
    Set,
    Record(TypeKey),
    Enum(TypeKey),
    Host(TypeKey),
    Trait(String),
    Function,
}

pub type NativeFunction = Arc<dyn Fn(&[Value]) -> VmResult<Value> + Send + Sync + 'static>;
pub type HostNativeFunction = Arc<
    dyn for<'host> Fn(&[Value], &mut HostExecution<'host>) -> VmResult<Value>
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone)]
pub struct NativeFunctionEntry {
    pub desc: NativeFunctionDesc,
    pub function: NativeFunction,
}

impl NativeFunctionEntry {
    #[must_use]
    pub fn new(
        desc: NativeFunctionDesc,
        function: impl Fn(&[Value]) -> VmResult<Value> + Send + Sync + 'static,
    ) -> Self {
        Self {
            desc,
            function: Arc::new(function),
        }
    }
}

#[derive(Clone)]
pub struct HostNativeFunctionEntry {
    pub desc: NativeFunctionDesc,
    pub function: HostNativeFunction,
}

impl HostNativeFunctionEntry {
    #[must_use]
    pub fn new(
        desc: NativeFunctionDesc,
        function: impl for<'host> Fn(&[Value], &mut HostExecution<'host>) -> VmResult<Value>
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
