use std::sync::Arc;

use vela_common::{FunctionId, Span};
use vela_reflect::registry::{AttrMap, TypeKey};
use vela_vm::HostExecution;
use vela_vm::error::VmResult;
use vela_vm::owned_value::OwnedValue;

use crate::context::NativeCallContext;
use crate::permission::Capability;

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
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
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
            attrs: AttrMap::new(),
            source_span: None,
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

    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(name, value);
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
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
    pub reads_time: bool,
    pub uses_random: bool,
    pub reads_io: bool,
    pub writes_io: bool,
    pub reads_reflection: bool,
    pub writes_reflection: bool,
    pub calls_reflection: bool,
}

impl EffectSet {
    #[must_use]
    pub const fn pure() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: false,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub const fn host_read() -> Self {
        Self {
            reads_host: true,
            writes_host: false,
            emits_events: false,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub const fn host_write() -> Self {
        Self {
            reads_host: true,
            writes_host: true,
            emits_events: false,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub const fn event_emit() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: true,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub const fn time() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: false,
            reads_time: true,
            uses_random: false,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub const fn random() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: false,
            reads_time: false,
            uses_random: true,
            reads_io: false,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub const fn io_read() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: false,
            reads_time: false,
            uses_random: false,
            reads_io: true,
            writes_io: false,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    #[must_use]
    pub const fn io_write() -> Self {
        Self {
            reads_host: false,
            writes_host: false,
            emits_events: false,
            reads_time: false,
            uses_random: false,
            reads_io: false,
            writes_io: true,
            reads_reflection: false,
            writes_reflection: false,
            calls_reflection: false,
        }
    }

    pub fn required_capabilities(&self) -> impl Iterator<Item = Capability> {
        [
            (self.reads_host && !self.writes_host, Capability::HostRead),
            (self.writes_host, Capability::HostWrite),
            (self.emits_events, Capability::EventEmit),
            (self.reads_time, Capability::Time),
            (self.uses_random, Capability::Random),
            (self.reads_io, Capability::IoRead),
            (self.writes_io, Capability::IoWrite),
            (self.reads_reflection, Capability::ReflectionRead),
            (self.writes_reflection, Capability::ReflectionWrite),
            (self.calls_reflection, Capability::ReflectionCall),
        ]
        .into_iter()
        .filter_map(|(required, capability)| required.then_some(capability))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionAccess {
    pub public: bool,
    pub reflect_visible: bool,
    pub reflect_callable: bool,
}

impl FunctionAccess {
    #[must_use]
    pub fn public() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn private() -> Self {
        Self {
            public: false,
            reflect_visible: false,
            reflect_callable: false,
        }
    }

    #[must_use]
    pub fn reflect_callable(mut self, reflect_callable: bool) -> Self {
        self.reflect_callable = reflect_callable;
        self
    }

    #[must_use]
    pub fn reflect_visible(mut self, reflect_visible: bool) -> Self {
        self.reflect_visible = reflect_visible;
        self
    }
}

impl Default for FunctionAccess {
    fn default() -> Self {
        Self {
            public: true,
            reflect_visible: true,
            reflect_callable: false,
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
    PathProxy,
    Record(TypeKey),
    Enum(TypeKey),
    Host(TypeKey),
    Trait(String),
    Function,
}

pub type NativeFunction =
    Arc<dyn Fn(&[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static>;
pub type HostNativeFunction = Arc<
    dyn for<'host> Fn(&[OwnedValue], &mut HostExecution<'host>) -> VmResult<OwnedValue>
        + Send
        + Sync
        + 'static,
>;
pub type ContextHostNativeFunction = Arc<
    dyn for<'ctx, 'host> Fn(
            &[OwnedValue],
            &mut NativeCallContext<'ctx, 'host>,
        ) -> VmResult<OwnedValue>
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
        function: impl Fn(&[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static,
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
        function: impl for<'host> Fn(&[OwnedValue], &mut HostExecution<'host>) -> VmResult<OwnedValue>
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

#[derive(Clone)]
pub struct ContextHostNativeFunctionEntry {
    pub desc: NativeFunctionDesc,
    pub function: ContextHostNativeFunction,
}

impl ContextHostNativeFunctionEntry {
    #[must_use]
    pub fn new(
        desc: NativeFunctionDesc,
        function: impl for<'ctx, 'host> Fn(
            &[OwnedValue],
            &mut NativeCallContext<'ctx, 'host>,
        ) -> VmResult<OwnedValue>
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
