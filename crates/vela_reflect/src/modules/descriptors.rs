use vela_common::{FunctionId, Span};

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
    pub public: bool,
    pub effects: FunctionEffectSet,
    pub access: FunctionAccess,
    pub origin: DeclOrigin,
    pub docs: Option<String>,
    pub attrs: AttrMap,
    pub source_span: Option<Span>,
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
            public: true,
            effects: FunctionEffectSet::default(),
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleExportDesc {
    pub name: String,
    pub kind: ModuleExportKind,
    pub function: Option<FunctionId>,
}

impl ModuleExportDesc {
    #[must_use]
    pub fn function(name: impl Into<String>, function: FunctionId) -> Self {
        Self {
            name: name.into(),
            kind: ModuleExportKind::Function,
            function: Some(function),
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
}
