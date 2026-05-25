use std::collections::BTreeMap;

use vela_common::Span;
use vela_reflect::{FunctionDesc, MethodDesc, SchemaHash, TypeRegistry};

use crate::{HotReloadError, HotReloadErrorKind, HotReloadResult};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HotReloadAbi {
    schemas: BTreeMap<String, SchemaAbi>,
    functions: BTreeMap<String, FunctionAbi>,
    methods: BTreeMap<(String, String), MethodAbi>,
}

impl HotReloadAbi {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_registry(registry: &TypeRegistry) -> Self {
        let mut manifest = Self::empty();
        for type_desc in registry.types() {
            if let Some(schema_hash) = type_desc.schema_hash {
                let mut schema = SchemaAbi::new(type_desc.key.name.clone(), schema_hash);
                if let Some(source_span) = type_desc.source_span {
                    schema = schema.source_span(source_span);
                }
                manifest = manifest.schema(schema);
            }
            for method in &type_desc.methods {
                manifest = manifest.method(MethodAbi::from_method(&type_desc.key.name, method));
            }
        }
        for function in registry.functions() {
            manifest = manifest.function(FunctionAbi::from_function(function));
        }
        manifest
    }

    #[must_use]
    pub fn schema(mut self, schema: SchemaAbi) -> Self {
        self.schemas.insert(schema.type_name.clone(), schema);
        self
    }

    #[must_use]
    pub fn function(mut self, function: FunctionAbi) -> Self {
        self.functions.insert(function.name.clone(), function);
        self
    }

    #[must_use]
    pub fn method(mut self, method: MethodAbi) -> Self {
        self.methods
            .insert((method.type_name.clone(), method.name.clone()), method);
        self
    }

    pub(crate) fn ensure_compatible_update(&self, next: &Self) -> HotReloadResult<()> {
        for (type_name, old_schema) in &self.schemas {
            let Some(new_schema) = next.schemas.get(type_name) else {
                return Err(HotReloadError::new(HotReloadErrorKind::RemovedSchema {
                    type_name: type_name.clone(),
                    old_hash: old_schema.hash,
                    source_span: old_schema.source_span.map(Box::new),
                }));
            };
            if old_schema.hash != new_schema.hash {
                return Err(HotReloadError::new(HotReloadErrorKind::ChangedSchema {
                    type_name: type_name.clone(),
                    old_hash: old_schema.hash,
                    new_hash: new_schema.hash,
                    source_span: new_schema.source_span.map(Box::new),
                }));
            }
        }

        for (function, old_function) in &self.functions {
            let Some(new_function) = next.functions.get(function) else {
                return Err(HotReloadError::new(
                    HotReloadErrorKind::RemovedFunctionAbi {
                        function: function.clone(),
                        source_span: old_function.source_span.map(Box::new),
                    },
                ));
            };
            old_function.ensure_compatible(new_function)?;
        }

        for ((type_name, method), old_method) in &self.methods {
            let Some(new_method) = next.methods.get(&(type_name.clone(), method.clone())) else {
                return Err(HotReloadError::new(HotReloadErrorKind::RemovedMethodAbi {
                    type_name: type_name.clone(),
                    method: method.clone(),
                    source_span: old_method.source_span.map(Box::new),
                }));
            };
            old_method.ensure_compatible(new_method)?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaAbi {
    pub type_name: String,
    pub hash: u64,
    pub source_span: Option<Span>,
}

impl SchemaAbi {
    #[must_use]
    pub fn new(type_name: impl Into<String>, hash: SchemaHash) -> Self {
        Self {
            type_name: type_name.into(),
            hash: hash.get(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionAbi {
    pub name: String,
    pub effects: EffectAbi,
    pub access: AccessAbi,
    pub source_span: Option<Span>,
}

impl FunctionAbi {
    #[must_use]
    pub fn new(name: impl Into<String>, effects: EffectAbi, access: AccessAbi) -> Self {
        Self {
            name: name.into(),
            effects,
            access,
            source_span: None,
        }
    }

    #[must_use]
    pub fn from_function(function: &FunctionDesc) -> Self {
        let mut abi = Self::new(
            function.name.clone(),
            EffectAbi::new(
                function.effects.reads_host,
                function.effects.writes_host,
                function.effects.emits_events,
            ),
            AccessAbi::new(
                function.access.public,
                function.access.reflect_visible,
                function.access.required_permissions().to_vec(),
            ),
        );
        if let Some(source_span) = function.source_span {
            abi = abi.source_span(source_span);
        }
        abi
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }

    fn ensure_compatible(&self, next: &Self) -> HotReloadResult<()> {
        if self.effects != next.effects {
            return Err(HotReloadError::new(
                HotReloadErrorKind::ChangedFunctionEffects {
                    function: self.name.clone(),
                    old: self.effects.clone(),
                    new: next.effects.clone(),
                    source_span: next.source_span.map(Box::new),
                },
            ));
        }
        if self.access != next.access {
            return Err(HotReloadError::new(
                HotReloadErrorKind::ChangedFunctionAccess {
                    function: self.name.clone(),
                    old: self.access.clone(),
                    new: next.access.clone(),
                    source_span: next.source_span.map(Box::new),
                },
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAbi {
    pub type_name: String,
    pub name: String,
    pub effects: EffectAbi,
    pub access: AccessAbi,
    pub source_span: Option<Span>,
}

impl MethodAbi {
    #[must_use]
    pub fn new(
        type_name: impl Into<String>,
        name: impl Into<String>,
        effects: EffectAbi,
        access: AccessAbi,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            name: name.into(),
            effects,
            access,
            source_span: None,
        }
    }

    #[must_use]
    pub fn from_method(type_name: &str, method: &MethodDesc) -> Self {
        let mut abi = Self::new(
            type_name,
            method.name.clone(),
            EffectAbi::new(
                method.effects.reads_host,
                method.effects.writes_host,
                method.effects.emits_events,
            ),
            AccessAbi::new(
                method.access.public,
                method.access.reflect_callable,
                method.access.required_permissions().to_vec(),
            ),
        );
        if let Some(source_span) = method.source_span {
            abi = abi.source_span(source_span);
        }
        abi
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }

    fn ensure_compatible(&self, next: &Self) -> HotReloadResult<()> {
        if self.effects != next.effects {
            return Err(HotReloadError::new(
                HotReloadErrorKind::ChangedMethodEffects {
                    type_name: self.type_name.clone(),
                    method: self.name.clone(),
                    old: self.effects.clone(),
                    new: next.effects.clone(),
                    source_span: next.source_span.map(Box::new),
                },
            ));
        }
        if self.access != next.access {
            return Err(HotReloadError::new(
                HotReloadErrorKind::ChangedMethodAccess {
                    type_name: self.type_name.clone(),
                    method: self.name.clone(),
                    old: self.access.clone(),
                    new: next.access.clone(),
                    source_span: next.source_span.map(Box::new),
                },
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectAbi {
    pub reads_host: bool,
    pub writes_host: bool,
    pub emits_events: bool,
}

impl EffectAbi {
    #[must_use]
    pub const fn pure() -> Self {
        Self::new(false, false, false)
    }

    #[must_use]
    pub const fn host_read() -> Self {
        Self::new(true, false, false)
    }

    #[must_use]
    pub const fn host_write() -> Self {
        Self::new(true, true, false)
    }

    #[must_use]
    pub const fn event_emit() -> Self {
        Self::new(false, false, true)
    }

    #[must_use]
    pub const fn new(reads_host: bool, writes_host: bool, emits_events: bool) -> Self {
        Self {
            reads_host,
            writes_host,
            emits_events,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessAbi {
    pub public: bool,
    pub reflective: bool,
    pub required_permissions: Vec<String>,
}

impl AccessAbi {
    #[must_use]
    pub fn public() -> Self {
        Self::new(true, true, Vec::new())
    }

    #[must_use]
    pub fn new(public: bool, reflective: bool, mut required_permissions: Vec<String>) -> Self {
        required_permissions.sort();
        required_permissions.dedup();
        Self {
            public,
            reflective,
            required_permissions,
        }
    }
}
