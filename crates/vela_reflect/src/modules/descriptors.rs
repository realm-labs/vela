use vela_common::{CallableAsyncness, Span};
use vela_def::{FunctionId, StateId};

use crate::{
    access::{FunctionAccess, FunctionEffectSet},
    registry::AttrMap,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclOrigin {
    Host,
    Script,
}

impl DeclOrigin {
    #[must_use]
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Script => "script",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionParamDesc {
    pub name: String,
    pub type_hint: Option<String>,
    pub has_default: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetachedValueMode {
    Detachable,
    RuntimeChecked,
}

impl DetachedValueMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Detachable => "detachable",
            Self::RuntimeChecked => "runtime_checked",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetachedTargetDesc {
    pub parameter_contracts: Vec<String>,
    pub parameter_modes: Vec<DetachedValueMode>,
    pub result_contract: String,
    pub result_mode: DetachedValueMode,
    pub effects: FunctionEffectSet,
    pub requires_service_generation: bool,
}

impl FunctionParamDesc {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_hint: None,
            has_default: false,
        }
    }

    #[must_use]
    pub fn type_hint(mut self, type_hint: impl Into<String>) -> Self {
        self.type_hint = Some(type_hint.into());
        self
    }

    #[must_use]
    pub fn defaulted(mut self, has_default: bool) -> Self {
        self.has_default = has_default;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionDesc {
    pub id: FunctionId,
    pub name: String,
    pub module: Option<String>,
    pub params: Vec<FunctionParamDesc>,
    pub return_type: Option<String>,
    pub asyncness: CallableAsyncness,
    pub public: bool,
    pub effects: FunctionEffectSet,
    pub detached_target: Option<DetachedTargetDesc>,
    pub access: FunctionAccess,
    pub origin: DeclOrigin,
    pub docs: Option<String>,
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateStorage {
    Vm,
    Extern,
}

impl StateStorage {
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Vm => "vm",
            Self::Extern => "extern",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateDesc {
    pub id: StateId,
    pub name: String,
    pub module: Option<String>,
    pub public: bool,
    pub storage: StateStorage,
    pub type_contract: String,
    pub has_initializer: bool,
    pub origin: DeclOrigin,
    pub source_span: Option<Span>,
}

impl StateDesc {
    #[must_use]
    pub fn new(id: StateId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            module: None,
            public: false,
            storage: StateStorage::Vm,
            type_contract: String::new(),
            has_initializer: false,
            origin: DeclOrigin::Host,
            source_span: None,
        }
    }

    #[must_use]
    pub fn module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    #[must_use]
    pub const fn public(mut self, public: bool) -> Self {
        self.public = public;
        self
    }

    #[must_use]
    pub const fn storage(mut self, storage: StateStorage) -> Self {
        self.storage = storage;
        self
    }

    #[must_use]
    pub fn type_contract(mut self, type_contract: impl Into<String>) -> Self {
        self.type_contract = type_contract.into();
        self
    }

    #[must_use]
    pub const fn initializer(mut self, has_initializer: bool) -> Self {
        self.has_initializer = has_initializer;
        self
    }

    #[must_use]
    pub const fn origin(mut self, origin: DeclOrigin) -> Self {
        self.origin = origin;
        self
    }

    #[must_use]
    pub const fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }
}

impl FunctionDesc {
    #[must_use]
    pub fn new(id: FunctionId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            module: None,
            params: Vec::new(),
            return_type: None,
            asyncness: CallableAsyncness::Sync,
            public: true,
            effects: FunctionEffectSet::default(),
            detached_target: None,
            access: FunctionAccess::default(),
            origin: DeclOrigin::Host,
            docs: None,
            attrs: AttrMap::new(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    #[must_use]
    pub fn param(mut self, param: FunctionParamDesc) -> Self {
        self.params.push(param);
        self
    }

    #[must_use]
    pub fn return_type(mut self, return_type: impl Into<String>) -> Self {
        self.return_type = Some(return_type.into());
        self
    }

    #[must_use]
    pub const fn asyncness(mut self, asyncness: CallableAsyncness) -> Self {
        self.asyncness = asyncness;
        self
    }

    #[must_use]
    pub fn public(mut self, public: bool) -> Self {
        self.public = public;
        self.access.public = public;
        self
    }

    #[must_use]
    pub fn effects(mut self, effects: FunctionEffectSet) -> Self {
        self.effects = effects;
        self
    }

    #[must_use]
    pub fn access(mut self, access: FunctionAccess) -> Self {
        self.public = access.public;
        self.access = access;
        self
    }

    #[must_use]
    pub fn origin(mut self, origin: DeclOrigin) -> Self {
        self.origin = origin;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleExportKind {
    Function,
    State,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleExportDesc {
    pub name: String,
    pub kind: ModuleExportKind,
    pub function: Option<FunctionId>,
    pub state: Option<StateId>,
}

impl ModuleExportDesc {
    #[must_use]
    pub fn function(name: impl Into<String>, function: FunctionId) -> Self {
        Self {
            name: name.into(),
            kind: ModuleExportKind::Function,
            function: Some(function),
            state: None,
        }
    }

    #[must_use]
    pub fn state(name: impl Into<String>, state: StateId) -> Self {
        Self {
            name: name.into(),
            kind: ModuleExportKind::State,
            function: None,
            state: Some(state),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDesc {
    pub name: String,
    pub exports: Vec<ModuleExportDesc>,
    pub origin: DeclOrigin,
    pub docs: Option<String>,
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
}

impl ModuleDesc {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            exports: Vec::new(),
            origin: DeclOrigin::Host,
            docs: None,
            attrs: AttrMap::new(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn origin(mut self, origin: DeclOrigin) -> Self {
        self.origin = origin;
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

    pub(crate) fn export_function(&mut self, name: impl Into<String>, function: FunctionId) {
        let name = name.into();
        if self
            .exports
            .iter()
            .any(|export| export.kind == ModuleExportKind::Function && export.name == name)
        {
            return;
        }
        self.exports
            .push(ModuleExportDesc::function(name, function));
    }

    pub(crate) fn export_state(&mut self, name: impl Into<String>, state: StateId) {
        let name = name.into();
        if self
            .exports
            .iter()
            .any(|export| export.kind == ModuleExportKind::State && export.name == name)
        {
            return;
        }
        self.exports.push(ModuleExportDesc::state(name, state));
    }
}
