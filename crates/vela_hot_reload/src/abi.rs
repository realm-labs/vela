use std::collections::BTreeMap;

use vela_common::Span;
use vela_reflect::{
    FunctionDesc, FunctionParamDesc, MethodDesc, MethodParamDesc, SchemaHash, TraitDesc,
    TraitMethodDesc, TypeRegistry,
};

use crate::{HotReloadError, HotReloadErrorKind, HotReloadResult};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HotReloadAbi {
    schemas: BTreeMap<String, SchemaAbi>,
    functions: BTreeMap<String, FunctionAbi>,
    methods: BTreeMap<(String, String), MethodAbi>,
    traits: BTreeMap<String, TraitAbi>,
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
        for trait_desc in registry.traits() {
            manifest = manifest.trait_abi(TraitAbi::from_trait(trait_desc));
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

    #[must_use]
    pub fn trait_abi(mut self, trait_abi: TraitAbi) -> Self {
        self.traits.insert(trait_abi.name.clone(), trait_abi);
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

        for (trait_name, old_trait) in &self.traits {
            let Some(new_trait) = next.traits.get(trait_name) else {
                return Err(HotReloadError::new(HotReloadErrorKind::RemovedTraitAbi {
                    trait_name: trait_name.clone(),
                    source_span: old_trait.source_span.map(Box::new),
                }));
            };
            old_trait.ensure_compatible(new_trait)?;
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
    pub params: Vec<ParamAbi>,
    pub return_type: Option<String>,
    pub event: Option<String>,
    pub effects: EffectAbi,
    pub access: AccessAbi,
    pub source_span: Option<Span>,
}

impl FunctionAbi {
    #[must_use]
    pub fn new(name: impl Into<String>, effects: EffectAbi, access: AccessAbi) -> Self {
        Self {
            name: name.into(),
            params: Vec::new(),
            return_type: None,
            event: None,
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
        if let Some(event) = function.attrs.get("event") {
            abi = abi.event(event);
        }
        for param in &function.params {
            abi = abi.param(ParamAbi::from_function_param(param));
        }
        if let Some(return_type) = &function.return_type {
            abi = abi.return_type(return_type.clone());
        }
        if let Some(source_span) = function.source_span {
            abi = abi.source_span(source_span);
        }
        abi
    }

    #[must_use]
    pub fn event(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }

    #[must_use]
    pub fn param(mut self, param: ParamAbi) -> Self {
        self.params.push(param);
        self
    }

    #[must_use]
    pub fn return_type(mut self, return_type: impl Into<String>) -> Self {
        self.return_type = Some(return_type.into());
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }

    fn ensure_compatible(&self, next: &Self) -> HotReloadResult<()> {
        ensure_function_params_compatible(self, next)?;
        if self.return_type != next.return_type {
            return Err(HotReloadError::new(
                HotReloadErrorKind::ChangedFunctionReturnAbi {
                    function: self.name.clone(),
                    old: self.return_type.clone(),
                    new: next.return_type.clone(),
                    source_span: next.source_span.map(Box::new),
                },
            ));
        }
        if self.event != next.event {
            return Err(HotReloadError::new(
                HotReloadErrorKind::ChangedFunctionEvent {
                    function: self.name.clone(),
                    old: self.event.clone(),
                    new: next.event.clone(),
                    source_span: next.source_span.map(Box::new),
                },
            ));
        }
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
pub struct ParamAbi {
    pub name: String,
    pub type_hint: Option<String>,
    pub has_default: bool,
}

impl ParamAbi {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_hint: None,
            has_default: false,
        }
    }

    #[must_use]
    pub fn from_function_param(param: &FunctionParamDesc) -> Self {
        let mut abi = Self::new(param.name.clone()).defaulted(param.has_default);
        if let Some(type_hint) = &param.type_hint {
            abi = abi.type_hint(type_hint.clone());
        }
        abi
    }

    #[must_use]
    pub fn from_method_param(param: &MethodParamDesc) -> Self {
        let mut abi = Self::new(param.name.clone()).defaulted(param.has_default);
        if let Some(type_hint) = &param.type_hint {
            abi = abi.type_hint(type_hint.clone());
        }
        abi
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

fn ensure_function_params_compatible(
    old_function: &FunctionAbi,
    new_function: &FunctionAbi,
) -> HotReloadResult<()> {
    if new_function.params.len() < old_function.params.len() {
        return Err(HotReloadError::new(
            HotReloadErrorKind::DeletedFunctionParameters {
                function: old_function.name.clone(),
                old: param_names(&old_function.params),
                new: param_names(&new_function.params),
            },
        ));
    }

    let changed = old_function
        .params
        .iter()
        .zip(&new_function.params)
        .any(|(old, new)| old != new);
    if changed {
        return Err(HotReloadError::new(
            HotReloadErrorKind::ChangedFunctionParameterAbi {
                function: old_function.name.clone(),
                old: old_function.params.clone(),
                new: new_function.params.clone(),
                source_span: new_function.source_span.map(Box::new),
            },
        ));
    }

    let added_required = new_function
        .params
        .iter()
        .skip(old_function.params.len())
        .filter(|param| !param.has_default)
        .map(|param| param.name.clone())
        .collect::<Vec<_>>();
    if !added_required.is_empty() {
        return Err(HotReloadError::new(
            HotReloadErrorKind::AddedFunctionParametersWithoutDefaults {
                function: old_function.name.clone(),
                added: added_required,
            },
        ));
    }

    Ok(())
}

fn param_names(params: &[ParamAbi]) -> Vec<String> {
    params.iter().map(|param| param.name.clone()).collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodAbi {
    pub type_name: String,
    pub name: String,
    pub params: Vec<ParamAbi>,
    pub return_type: Option<String>,
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
            params: Vec::new(),
            return_type: None,
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
        for param in &method.params {
            abi = abi.param(ParamAbi::from_method_param(param));
        }
        if let Some(return_type) = &method.return_type {
            abi = abi.return_type(return_type.clone());
        }
        if let Some(source_span) = method.source_span {
            abi = abi.source_span(source_span);
        }
        abi
    }

    #[must_use]
    pub fn param(mut self, param: ParamAbi) -> Self {
        self.params.push(param);
        self
    }

    #[must_use]
    pub fn return_type(mut self, return_type: impl Into<String>) -> Self {
        self.return_type = Some(return_type.into());
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }

    fn ensure_compatible(&self, next: &Self) -> HotReloadResult<()> {
        ensure_method_params_compatible(self, next)?;
        if self.return_type != next.return_type {
            return Err(HotReloadError::new(
                HotReloadErrorKind::ChangedMethodReturnAbi {
                    type_name: self.type_name.clone(),
                    method: self.name.clone(),
                    old: self.return_type.clone(),
                    new: next.return_type.clone(),
                    source_span: next.source_span.map(Box::new),
                },
            ));
        }
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

fn ensure_method_params_compatible(
    old_method: &MethodAbi,
    new_method: &MethodAbi,
) -> HotReloadResult<()> {
    let changed_existing = new_method.params.len() < old_method.params.len()
        || old_method
            .params
            .iter()
            .zip(&new_method.params)
            .any(|(old, new)| old != new);
    if changed_existing {
        return Err(HotReloadError::new(
            HotReloadErrorKind::ChangedMethodParameterAbi {
                type_name: old_method.type_name.clone(),
                method: old_method.name.clone(),
                old: old_method.params.clone(),
                new: new_method.params.clone(),
                source_span: new_method.source_span.map(Box::new),
            },
        ));
    }

    let added_required = new_method
        .params
        .iter()
        .skip(old_method.params.len())
        .any(|param| !param.has_default);
    if added_required {
        return Err(HotReloadError::new(
            HotReloadErrorKind::ChangedMethodParameterAbi {
                type_name: old_method.type_name.clone(),
                method: old_method.name.clone(),
                old: old_method.params.clone(),
                new: new_method.params.clone(),
                source_span: new_method.source_span.map(Box::new),
            },
        ));
    }

    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitAbi {
    pub name: String,
    pub methods: Vec<TraitMethodAbi>,
    pub source_span: Option<Span>,
}

impl TraitAbi {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            methods: Vec::new(),
            source_span: None,
        }
    }

    #[must_use]
    pub fn from_trait(trait_desc: &TraitDesc) -> Self {
        let mut abi = Self::new(trait_desc.name.clone());
        for method in &trait_desc.methods {
            abi = abi.method(TraitMethodAbi::from_method(method));
        }
        if let Some(source_span) = trait_desc.source_span {
            abi = abi.source_span(source_span);
        }
        abi
    }

    #[must_use]
    pub fn method(mut self, method: TraitMethodAbi) -> Self {
        self.methods.push(method);
        self
    }

    #[must_use]
    pub fn source_span(mut self, source_span: Span) -> Self {
        self.source_span = Some(source_span);
        self
    }

    fn ensure_compatible(&self, next: &Self) -> HotReloadResult<()> {
        let next_methods = next
            .methods
            .iter()
            .map(|method| (method.name.as_str(), method))
            .collect::<BTreeMap<_, _>>();
        let changed_existing = self.methods.iter().any(|old| {
            next_methods
                .get(old.name.as_str())
                .is_none_or(|new| *new != old)
        });
        let old_methods = self
            .methods
            .iter()
            .map(|method| method.name.as_str())
            .collect::<Vec<_>>();
        let added_required = next
            .methods
            .iter()
            .filter(|method| !old_methods.contains(&method.name.as_str()))
            .any(|method| !method.has_default);
        if changed_existing || added_required {
            return Err(HotReloadError::new(HotReloadErrorKind::ChangedTraitAbi {
                trait_name: self.name.clone(),
                old: self.methods.clone(),
                new: next.methods.clone(),
                source_span: next.source_span.map(Box::new),
            }));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitMethodAbi {
    pub id: u32,
    pub name: String,
    pub params: Vec<ParamAbi>,
    pub return_type: Option<String>,
    pub has_default: bool,
}

impl TraitMethodAbi {
    #[must_use]
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            params: Vec::new(),
            return_type: None,
            has_default: false,
        }
    }

    #[must_use]
    pub fn from_method(method: &TraitMethodDesc) -> Self {
        let mut abi = Self::new(method.id.get(), method.name.clone()).defaulted(method.has_default);
        for param in &method.params {
            abi = abi.param(ParamAbi::from_method_param(param));
        }
        if let Some(return_type) = &method.return_type {
            abi = abi.return_type(return_type.clone());
        }
        abi
    }

    #[must_use]
    pub fn param(mut self, param: ParamAbi) -> Self {
        self.params.push(param);
        self
    }

    #[must_use]
    pub fn return_type(mut self, return_type: impl Into<String>) -> Self {
        self.return_type = Some(return_type.into());
        self
    }

    #[must_use]
    pub fn defaulted(mut self, has_default: bool) -> Self {
        self.has_default = has_default;
        self
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
