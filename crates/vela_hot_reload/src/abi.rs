use std::collections::BTreeMap;

use vela_common::Span;
use vela_hir::module_graph::ModuleGraph;
use vela_reflect::modules::{DeclOrigin, FunctionDesc, FunctionParamDesc};
use vela_reflect::registry::{
    MethodDesc, MethodParamDesc, TraitDesc, TraitMethodDesc, TypeRegistry,
};
use vela_registry::TypeHintDef;

use crate::error::{HotReloadError, HotReloadErrorKind, HotReloadResult};
use crate::module_abi::ModuleAbi;
use crate::schema_abi::SchemaAbi;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HotReloadAbi {
    schemas: BTreeMap<String, SchemaAbi>,
    functions: BTreeMap<String, FunctionAbi>,
    methods: BTreeMap<(String, String), MethodAbi>,
    traits: BTreeMap<String, TraitAbi>,
    modules: BTreeMap<String, ModuleAbi>,
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
            if let Some(schema) = SchemaAbi::from_type(type_desc) {
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
        for module in registry.modules() {
            manifest = manifest.module(ModuleAbi::from_module(module));
        }
        manifest
    }

    #[must_use]
    pub fn with_script_metadata(self, graph: &ModuleGraph) -> Self {
        let mut registry = TypeRegistry::new();
        registry.register_script_types(graph);
        registry.register_script_modules(graph);
        self.merge(Self::from_registry(&registry))
    }

    #[must_use]
    fn merge(mut self, other: Self) -> Self {
        self.schemas.extend(other.schemas);
        self.functions.extend(other.functions);
        self.methods.extend(other.methods);
        self.traits.extend(other.traits);
        self.modules.extend(other.modules);
        self
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

    #[must_use]
    pub fn module(mut self, module: ModuleAbi) -> Self {
        self.modules.insert(module.name.clone(), module);
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
            if old_schema.has_member_abi() || new_schema.has_member_abi() {
                old_schema.ensure_compatible(new_schema)?;
                continue;
            }
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
            let Some(new_function) = find_compatible_function(function, old_function, next) else {
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
            let Some(new_method) = find_compatible_method(type_name, method, old_method, next)
            else {
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

        for (module, old_module) in &self.modules {
            let Some(new_module) = next.modules.get(module) else {
                return Err(HotReloadError::new(HotReloadErrorKind::RemovedModuleAbi {
                    module: module.clone(),
                    source_span: old_module.source_span.map(Box::new),
                }));
            };
            old_module.ensure_compatible(new_module)?;
        }

        Ok(())
    }

    pub fn host_function_aliases(&self) -> impl Iterator<Item = (u128, &str)> + '_ {
        self.functions.values().filter_map(|function| {
            let id = function.id?;
            (function.origin == DeclOrigin::Host).then_some((id, function.name.as_str()))
        })
    }
}

fn find_compatible_function<'a>(
    function: &str,
    old_function: &FunctionAbi,
    next: &'a HotReloadAbi,
) -> Option<&'a FunctionAbi> {
    if let Some(new_function) = next.functions.get(function) {
        return Some(new_function);
    }
    let old_id = old_function.id?;
    if old_function.origin != DeclOrigin::Host {
        return None;
    }
    next.functions.values().find(|new_function| {
        new_function.origin == DeclOrigin::Host && new_function.id == Some(old_id)
    })
}

fn find_compatible_method<'a>(
    type_name: &str,
    method: &str,
    old_method: &MethodAbi,
    next: &'a HotReloadAbi,
) -> Option<&'a MethodAbi> {
    if let Some(new_method) = next.methods.get(&(type_name.to_owned(), method.to_owned())) {
        return Some(new_method);
    }
    let old_id = old_method.id?;
    if old_method.origin != DeclOrigin::Host {
        return None;
    }
    next.methods.values().find(|new_method| {
        new_method.type_name == type_name
            && new_method.origin == DeclOrigin::Host
            && new_method.id == Some(old_id)
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionAbi {
    pub id: Option<u128>,
    pub name: String,
    pub origin: DeclOrigin,
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
            id: None,
            origin: DeclOrigin::Host,
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
                function.effects.reads_time,
                function.effects.uses_random,
                function.effects.reads_io,
                function.effects.writes_io,
                function.effects.reads_reflection,
                function.effects.writes_reflection,
                function.effects.calls_reflection,
            ),
            AccessAbi::function(
                function.access.public,
                function.access.reflect_visible,
                function.access.reflect_callable,
            ),
        )
        .id(function.id.get())
        .origin(function.origin);
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
    pub fn id(mut self, id: u128) -> Self {
        self.id = Some(id);
        self
    }

    #[must_use]
    pub fn origin(mut self, origin: DeclOrigin) -> Self {
        self.origin = origin;
        self
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
        if self.origin == DeclOrigin::Host
            && next.origin == DeclOrigin::Host
            && self.id.is_some()
            && next.id.is_some()
            && self.id != next.id
        {
            return Err(HotReloadError::new(
                HotReloadErrorKind::RemovedFunctionAbi {
                    function: self.name.clone(),
                    source_span: self.source_span.map(Box::new),
                },
            ));
        }
        ensure_function_params_compatible(self, next)?;
        if !type_hints_compatible(self.return_type.as_deref(), next.return_type.as_deref()) {
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
        .any(|(old, new)| !params_compatible(old, new));
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
    pub id: Option<u128>,
    pub type_name: String,
    pub name: String,
    pub origin: DeclOrigin,
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
            id: None,
            type_name: type_name.into(),
            name: name.into(),
            origin: DeclOrigin::Host,
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
                method.effects.reads_time,
                method.effects.uses_random,
                method.effects.reads_io,
                method.effects.writes_io,
                method.effects.reads_reflection,
                method.effects.writes_reflection,
                method.effects.calls_reflection,
            ),
            AccessAbi::new(method.access.public, method.access.reflect_callable),
        )
        .id(method.id.get())
        .origin(method.origin);
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
    pub fn id(mut self, id: u128) -> Self {
        self.id = Some(id);
        self
    }

    #[must_use]
    pub fn origin(mut self, origin: DeclOrigin) -> Self {
        self.origin = origin;
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
        if self.origin == DeclOrigin::Host
            && next.origin == DeclOrigin::Host
            && self.id.is_some()
            && next.id.is_some()
            && self.id != next.id
        {
            return Err(HotReloadError::new(HotReloadErrorKind::RemovedMethodAbi {
                type_name: self.type_name.clone(),
                method: self.name.clone(),
                source_span: self.source_span.map(Box::new),
            }));
        }
        ensure_method_params_compatible(self, next)?;
        if !type_hints_compatible(self.return_type.as_deref(), next.return_type.as_deref()) {
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
            .any(|(old, new)| !params_compatible(old, new));
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
                .is_none_or(|new| !trait_methods_compatible(old, new))
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

fn params_compatible(old: &ParamAbi, new: &ParamAbi) -> bool {
    old.name == new.name
        && type_hints_compatible(old.type_hint.as_deref(), new.type_hint.as_deref())
        && old.has_default == new.has_default
}

pub(crate) fn trait_methods_compatible(old: &TraitMethodAbi, new: &TraitMethodAbi) -> bool {
    old.id == new.id
        && old.name == new.name
        && old.params.len() == new.params.len()
        && old
            .params
            .iter()
            .zip(&new.params)
            .all(|(old, new)| params_compatible(old, new))
        && type_hints_compatible(old.return_type.as_deref(), new.return_type.as_deref())
        && old.has_default == new.has_default
}

pub(crate) fn type_hints_compatible(old: Option<&str>, new: Option<&str>) -> bool {
    match (old.map(type_hint_key), new.map(type_hint_key)) {
        (None, None) => true,
        (Some(TypeHintKey::Parsed(old)), Some(TypeHintKey::Parsed(new))) => old == new,
        (Some(TypeHintKey::Raw(old)), Some(TypeHintKey::Raw(new))) => old == new,
        _ => false,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TypeHintKey<'a> {
    Parsed(TypeHintDef),
    Raw(&'a str),
}

fn type_hint_key(type_hint: &str) -> TypeHintKey<'_> {
    TypeHintDef::parse(type_hint)
        .map(TypeHintKey::Parsed)
        .unwrap_or(TypeHintKey::Raw(type_hint))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitMethodAbi {
    pub id: u128,
    pub name: String,
    pub params: Vec<ParamAbi>,
    pub return_type: Option<String>,
    pub has_default: bool,
}

impl TraitMethodAbi {
    #[must_use]
    pub fn new(id: u128, name: impl Into<String>) -> Self {
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
    pub reads_time: bool,
    pub uses_random: bool,
    pub reads_io: bool,
    pub writes_io: bool,
    pub reads_reflection: bool,
    pub writes_reflection: bool,
    pub calls_reflection: bool,
}

impl EffectAbi {
    #[must_use]
    pub const fn pure() -> Self {
        Self::new(
            false, false, false, false, false, false, false, false, false, false,
        )
    }

    #[must_use]
    pub const fn host_read() -> Self {
        Self::new(
            true, false, false, false, false, false, false, false, false, false,
        )
    }

    #[must_use]
    pub const fn host_write() -> Self {
        Self::new(
            true, true, false, false, false, false, false, false, false, false,
        )
    }

    #[must_use]
    pub const fn event_emit() -> Self {
        Self::new(
            false, false, true, false, false, false, false, false, false, false,
        )
    }

    #[must_use]
    pub const fn time() -> Self {
        Self::new(
            false, false, false, true, false, false, false, false, false, false,
        )
    }

    #[must_use]
    pub const fn random() -> Self {
        Self::new(
            false, false, false, false, true, false, false, false, false, false,
        )
    }

    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        reads_host: bool,
        writes_host: bool,
        emits_events: bool,
        reads_time: bool,
        uses_random: bool,
        reads_io: bool,
        writes_io: bool,
        reads_reflection: bool,
        writes_reflection: bool,
        calls_reflection: bool,
    ) -> Self {
        Self {
            reads_host,
            writes_host,
            emits_events,
            reads_time,
            uses_random,
            reads_io,
            writes_io,
            reads_reflection,
            writes_reflection,
            calls_reflection,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessAbi {
    pub public: bool,
    pub reflective: bool,
    pub callable: bool,
}

impl AccessAbi {
    #[must_use]
    pub fn public() -> Self {
        Self::new(true, true)
    }

    #[must_use]
    pub const fn new(public: bool, reflective: bool) -> Self {
        Self {
            public,
            reflective,
            callable: reflective,
        }
    }

    #[must_use]
    pub const fn function(public: bool, reflect_visible: bool, reflect_callable: bool) -> Self {
        Self {
            public,
            reflective: reflect_visible,
            callable: reflect_callable,
        }
    }
}
