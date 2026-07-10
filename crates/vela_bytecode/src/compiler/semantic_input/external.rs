use std::collections::BTreeMap;

use vela_analysis::callable::{
    CallableParameterFact, CallableParameterRequirementFact, CallableSignatureFact,
};
use vela_analysis::registry::{RegistryFacts, RegistryIndexCapabilityFact};
use vela_analysis::type_fact::TypeFact;
use vela_common::{Diagnostic, HostMethodId, HostTypeId};
use vela_def::{DefPath, FieldId, FunctionId, MethodId, TypeId, VariantId};
use vela_mir::{
    CompileFieldAccess, CompileFieldDescriptor, CompileFunctionAccess, CompileFunctionClass,
    CompileFunctionDescriptor, CompileMethodAccess, CompileMethodClass, CompileMethodDescriptor,
    CompileParameter, CompileParameterDefault, CompilePositionalPolicy, CompileSignature,
    CompileTypeClass, CompileTypeDescriptor, CompileVariantDescriptor, HostTypeTarget, MirEffect,
    MirSourceOrigin, MirTypeContract,
};
use vela_registry::{
    Def, DefinitionRegistry, FieldDef, FunctionDef, MethodDef, RegistryCompileView,
    RegistryDeclarationSlotError, RegistryDeclarationSlots, TypeDef, TypeHintDef, TypeKindDef,
    VariantDef,
};

use super::{GenerationBuilder, input_error, registry_input_error};
use crate::compiler::error::{CompileError, CompileErrorKind, CompileResult};
use crate::compiler::options::CompilerOptions;

pub(super) fn combined_registry(
    provided: Option<RegistryCompileView<'_>>,
) -> CompileResult<DefinitionRegistry> {
    let standard = vela_stdlib::standard_registry().map_err(|error| {
        CompileError::new(CompileErrorKind::RegistrySnapshot(error.to_string()))
    })?;
    let mut combined = DefinitionRegistry::new();
    if let Some(provided) = provided {
        for definition in provided.definitions() {
            combined.insert(definition.clone()).map_err(|error| {
                CompileError::new(CompileErrorKind::RegistrySnapshot(error.to_string()))
            })?;
        }
    }
    for definition in standard.compile_view().definitions() {
        if combined.get_by_path(definition.path()).is_some() {
            continue;
        }
        combined.insert(definition.clone()).map_err(|error| {
            CompileError::new(CompileErrorKind::RegistrySnapshot(error.to_string()))
        })?;
    }
    for definition in combined.compile_view().definitions() {
        let Def::Type(definition) = definition else {
            continue;
        };
        if definition.kind == TypeKindDef::Host
            && definition
                .host_runtime_id
                .is_some_and(|runtime| u64::try_from(runtime).is_err())
        {
            return Err(CompileError::new(CompileErrorKind::RegistrySnapshot(
                format!(
                    "host type `{}` has runtime ID outside the u64 HostTypeId range",
                    source_name(&definition.path)
                ),
            )));
        }
    }
    Ok(combined)
}

pub(super) fn include_policy_neutral_reflection_signatures(facts: &mut RegistryFacts) {
    for spec in vela_stdlib::reflection_native_specs() {
        facts.insert_function_signature(
            spec.source_name,
            CallableSignatureFact::new(
                spec.params.iter().map(|name| {
                    CallableParameterFact::new(
                        *name,
                        TypeFact::Unknown,
                        CallableParameterRequirementFact::Required,
                    )
                }),
                TypeFact::Unknown,
            ),
        );
    }
}

pub(super) fn apply_option_index_capabilities(
    facts: &mut RegistryFacts,
    options: &CompilerOptions,
    catalog: &ExternalCatalog,
) {
    for (owner, capability) in &options.host_index_capabilities {
        let key = capability
            .key_type
            .as_deref()
            .map_or(TypeFact::Unknown, |value| catalog.type_fact_text(value));
        let value = capability
            .value_type
            .as_deref()
            .map_or(TypeFact::Unknown, |value| catalog.type_fact_text(value));
        facts.insert_index_capability(RegistryIndexCapabilityFact {
            owner: owner.clone(),
            readable: capability.readable,
            writable: capability.writable,
            addable: capability.addable,
            removable: capability.removable,
            key,
            value,
        });
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct ExternalCatalog {
    functions: BTreeMap<FunctionId, FunctionDef>,
    functions_by_source: BTreeMap<String, Option<FunctionId>>,
    methods: BTreeMap<MethodId, MethodDef>,
    methods_by_owner_name: BTreeMap<(TypeId, String), MethodId>,
    types: BTreeMap<TypeId, TypeDef>,
    types_by_source: BTreeMap<String, Option<TypeId>>,
    fields: BTreeMap<FieldId, FieldDef>,
    variants: BTreeMap<VariantId, VariantDef>,
}

impl ExternalCatalog {
    pub(super) fn from_view(
        view: RegistryCompileView<'_>,
        declaration_slots: &RegistryDeclarationSlots,
    ) -> Result<Self, RegistryDeclarationSlotError> {
        let mut catalog = Self::default();
        for definition in view.definitions() {
            match definition {
                Def::Function(value) => {
                    let name = source_name(&value.path);
                    insert_unambiguous(&mut catalog.functions_by_source, name, value.id);
                    catalog.functions.insert(value.id, value.clone());
                }
                Def::Method(value) => {
                    catalog
                        .methods_by_owner_name
                        .insert((value.owner, value.path.name.clone()), value.id);
                    catalog.methods.insert(value.id, value.clone());
                }
                Def::Type(value) => {
                    let canonical = source_name(&value.path);
                    insert_unambiguous(&mut catalog.types_by_source, canonical.clone(), value.id);
                    if canonical != value.path.name {
                        insert_unambiguous(
                            &mut catalog.types_by_source,
                            value.path.name.clone(),
                            value.id,
                        );
                    }
                    catalog.types.insert(value.id, value.clone());
                }
                Def::Field(value) => {
                    let mut value = value.clone();
                    value.declaration_order = declaration_slots.field(value.id)?;
                    catalog.fields.insert(value.id, value);
                }
                Def::Variant(value) => {
                    let mut value = value.clone();
                    value.declaration_order = declaration_slots.variant(value.id)?;
                    catalog.variants.insert(value.id, value);
                }
                Def::Trait(_) => {}
            }
        }
        Ok(catalog)
    }

    pub(super) fn include_policy_neutral_reflection_manifest(&mut self) {
        for definition in vela_stdlib::reflection_native_specs().map(|spec| spec.def()) {
            let name = source_name(&definition.path);
            insert_unambiguous(&mut self.functions_by_source, name, definition.id);
            self.functions.insert(definition.id, definition);
        }
    }

    pub(super) fn function_by_source(&self, path: &str) -> Option<&FunctionDef> {
        self.functions_by_source
            .get(path)
            .copied()
            .flatten()
            .and_then(|id| self.functions.get(&id))
    }

    pub(super) fn function(&self, id: FunctionId) -> Option<&FunctionDef> {
        self.functions.get(&id)
    }

    pub(super) fn method(&self, id: MethodId) -> Option<&MethodDef> {
        self.methods.get(&id)
    }

    pub(super) fn method_by_owner_name(&self, owner: TypeId, name: &str) -> Option<&MethodDef> {
        self.methods_by_owner_name
            .get(&(owner, name.to_owned()))
            .and_then(|id| self.methods.get(id))
    }

    pub(super) fn ty(&self, id: TypeId) -> Option<&TypeDef> {
        self.types.get(&id)
    }

    pub(super) fn field(&self, id: FieldId) -> Option<&FieldDef> {
        self.fields.get(&id)
    }

    pub(super) fn fields_for_owner(&self, owner: TypeId) -> Vec<&FieldDef> {
        self.fields
            .values()
            .filter(|field| field.owner == owner)
            .collect()
    }

    pub(super) fn field_by_owner_name(
        &self,
        owner: TypeId,
        variant: Option<VariantId>,
        name: &str,
    ) -> Option<&FieldDef> {
        self.fields.values().find(|field| {
            field.owner == owner && field.variant == variant && field.path.name == name
        })
    }

    pub(super) fn variant(&self, id: VariantId) -> Option<&VariantDef> {
        self.variants.get(&id)
    }

    pub(super) fn variant_by_owner_name(&self, owner: TypeId, name: &str) -> Option<&VariantDef> {
        self.variants
            .values()
            .find(|variant| variant.owner == owner && variant.path.name == name)
    }

    pub(super) fn variants_for_owner(&self, owner: TypeId) -> Vec<&VariantDef> {
        self.variants
            .values()
            .filter(|variant| variant.owner == owner)
            .collect()
    }

    pub(super) fn type_fact_text(&self, text: &str) -> TypeFact {
        TypeHintDef::parse(text)
            .as_ref()
            .map_or(TypeFact::Unknown, |hint| self.type_fact(hint))
    }

    pub(super) fn type_fact(&self, hint: &TypeHintDef) -> TypeFact {
        super::schema::registry_hint_fact(hint, self)
    }

    pub(super) fn type_by_source(&self, name: &str) -> Option<&TypeDef> {
        self.types_by_source
            .get(name)
            .copied()
            .flatten()
            .and_then(|id| self.types.get(&id))
    }
}

impl GenerationBuilder<'_, '_> {
    pub(super) fn ensure_external_function(
        &mut self,
        function: FunctionId,
        origin: MirSourceOrigin,
    ) -> CompileResult<()> {
        if !self.inserted_external_functions.insert(function) {
            return Ok(());
        }
        let definition = self
            .catalog
            .function(function)
            .cloned()
            .ok_or_else(registry_input_error)?;
        let signature = external_signature(
            &definition.signature,
            definition.effects,
            &self.catalog,
            origin,
        )?;
        self.remember_signature_contracts(&signature, origin);
        let descriptor = CompileFunctionDescriptor {
            id: definition.id,
            class: if definition.path.package == "std" {
                CompileFunctionClass::Stdlib
            } else if definition.path.package == "host" {
                CompileFunctionClass::Native
            } else {
                CompileFunctionClass::Registry
            },
            canonical_symbol: source_name(&definition.path),
            debug_name: source_name(&definition.path),
            signature,
            access: CompileFunctionAccess::new(
                definition.access.public,
                definition.access.reflect_visible,
                definition.access.reflect_callable,
            ),
        };
        self.targets
            .insert_function_descriptor(descriptor, origin)
            .map_err(input_error)
    }

    pub(super) fn ensure_derived_native_function(
        &mut self,
        path: &str,
        origin: MirSourceOrigin,
    ) -> CompileResult<FunctionId> {
        let function = derived_native_function_id(path);
        if self.inserted_external_functions.insert(function) {
            self.targets
                .insert_function_descriptor(
                    CompileFunctionDescriptor {
                        id: function,
                        class: CompileFunctionClass::Native,
                        canonical_symbol: path.to_owned(),
                        debug_name: path.to_owned(),
                        signature: CompileSignature {
                            parameters: Vec::new(),
                            positional: CompilePositionalPolicy::RuntimeChecked,
                            return_contract: None,
                            effect: MirEffect::external_call(),
                        },
                        access: CompileFunctionAccess::new(true, true, false),
                    },
                    origin,
                )
                .map_err(input_error)?;
        }
        Ok(function)
    }

    pub(super) fn ensure_external_type(
        &mut self,
        type_id: TypeId,
        origin: MirSourceOrigin,
    ) -> CompileResult<()> {
        if !self.inserted_external_types.insert(type_id) {
            return Ok(());
        }
        let definition = self
            .catalog
            .ty(type_id)
            .cloned()
            .ok_or_else(registry_input_error)?;
        let source_name = source_name(&definition.path);
        let canonical_name = definition.path.canonical_name();
        let mut fields = self.catalog.fields_for_owner(type_id);
        fields.sort_by_key(|field| (field.declaration_order, field.path.name.clone()));
        let mut variants = self.catalog.variants_for_owner(type_id);
        variants.sort_by_key(|variant| (variant.declaration_order, variant.path.name.clone()));
        let shape = matches!(definition.kind, TypeKindDef::ScriptStruct).then(|| {
            let mut names = fields
                .iter()
                .filter(|field| field.variant.is_none())
                .map(|field| field.path.name.as_str())
                .collect::<Vec<_>>();
            names.sort_unstable();
            vela_common::script_shape_id(&source_name, names.into_iter())
        });
        let class = if definition.path.package == "std" {
            CompileTypeClass::Standard
        } else if definition.kind == TypeKindDef::Host {
            match definition.host_runtime_id.and_then(host_type_id) {
                Some(runtime) => CompileTypeClass::Host { runtime },
                None => CompileTypeClass::Registry,
            }
        } else {
            CompileTypeClass::Registry
        };
        self.targets
            .insert_type_descriptor(
                CompileTypeDescriptor {
                    id: definition.id,
                    canonical_name,
                    class,
                    shape,
                    fields: fields
                        .into_iter()
                        .filter(|field| field.variant.is_none())
                        .map(|field| field.id)
                        .collect(),
                    variants: variants.into_iter().map(|variant| variant.id).collect(),
                },
                origin,
            )
            .map_err(input_error)?;
        let variants = self
            .catalog
            .variants_for_owner(type_id)
            .into_iter()
            .map(|variant| variant.id)
            .collect::<Vec<_>>();
        let fields = self
            .catalog
            .fields_for_owner(type_id)
            .into_iter()
            .map(|field| field.id)
            .collect::<Vec<_>>();
        for variant in variants {
            self.ensure_external_variant(variant, origin)?;
        }
        for field in fields {
            self.ensure_external_field(field, origin)?;
        }
        Ok(())
    }

    pub(super) fn ensure_opaque_external_type(
        &mut self,
        name: &str,
        origin: MirSourceOrigin,
    ) -> CompileResult<TypeId> {
        let path = derived_external_type_path(name);
        let type_id = TypeId::from_def_id(path.id());
        if self.inserted_external_types.insert(type_id) {
            self.targets
                .insert_type_descriptor(
                    CompileTypeDescriptor {
                        id: type_id,
                        canonical_name: path.canonical_name(),
                        class: CompileTypeClass::OpaqueExternal,
                        shape: None,
                        fields: Vec::new(),
                        variants: Vec::new(),
                    },
                    origin,
                )
                .map_err(input_error)?;
            self.type_names.insert(type_id, name.to_owned());
        }
        Ok(type_id)
    }

    pub(super) fn ensure_external_variant(
        &mut self,
        variant: VariantId,
        origin: MirSourceOrigin,
    ) -> CompileResult<()> {
        if !self.inserted_external_variants.insert(variant) {
            return Ok(());
        }
        let definition = self
            .catalog
            .variant(variant)
            .cloned()
            .ok_or_else(registry_input_error)?;
        self.ensure_external_type(definition.owner, origin)?;
        let mut fields = self
            .catalog
            .fields_for_owner(definition.owner)
            .into_iter()
            .filter(|field| field.variant == Some(variant))
            .collect::<Vec<_>>();
        fields.sort_by_key(|field| (field.declaration_order, field.path.name.clone()));
        self.targets
            .insert_variant_descriptor(
                CompileVariantDescriptor {
                    id: definition.id,
                    owner: definition.owner,
                    name: definition.path.name.clone(),
                    fields: fields.into_iter().map(|field| field.id).collect(),
                    declaration_order: definition.declaration_order,
                },
                origin,
            )
            .map_err(input_error)?;
        let fields = self
            .catalog
            .fields_for_owner(definition.owner)
            .into_iter()
            .filter(|field| field.variant == Some(variant))
            .map(|field| field.id)
            .collect::<Vec<_>>();
        for field in fields {
            self.ensure_external_field(field, origin)?;
        }
        Ok(())
    }

    pub(super) fn ensure_external_field(
        &mut self,
        field: FieldId,
        origin: MirSourceOrigin,
    ) -> CompileResult<()> {
        if !self.inserted_external_fields.insert(field) {
            return Ok(());
        }
        let definition = self
            .catalog
            .field(field)
            .cloned()
            .ok_or_else(registry_input_error)?;
        self.ensure_external_type(definition.owner, origin)?;
        if let Some(variant) = definition.variant {
            self.ensure_external_variant(variant, origin)?;
        }
        let contract = definition
            .type_hint
            .as_ref()
            .map(|hint| external_hint_contract(hint, &self.catalog, origin))
            .transpose()?
            .flatten();
        if let Some(contract) = &contract {
            self.remember_contract(contract, origin);
        }
        self.targets
            .insert_field_descriptor(
                CompileFieldDescriptor {
                    id: definition.id,
                    owner: definition.owner,
                    variant: definition.variant,
                    name: definition.path.name.clone(),
                    contract,
                    declaration_order: definition.declaration_order,
                    access: field_access(&definition),
                    host_runtime: definition.host_runtime_id.map(FieldId::new),
                },
                origin,
            )
            .map_err(input_error)
    }

    pub(super) fn ensure_external_method(
        &mut self,
        method: MethodId,
        origin: MirSourceOrigin,
    ) -> CompileResult<()> {
        let definition = self
            .catalog
            .method(method)
            .cloned()
            .ok_or_else(registry_input_error)?;
        if !self
            .inserted_external_methods
            .insert((definition.owner, method))
        {
            return Ok(());
        }
        self.ensure_external_type(definition.owner, origin)?;
        let class = if let Some(runtime) = definition.host_runtime_id {
            CompileMethodClass::Host {
                runtime: HostMethodId::new(runtime),
            }
        } else if definition.path.package == "std" {
            CompileMethodClass::Value
        } else {
            CompileMethodClass::Registry
        };
        let signature = external_signature(
            &definition.signature,
            definition.effects,
            &self.catalog,
            origin,
        )?;
        self.remember_signature_contracts(&signature, origin);
        self.targets
            .insert_method_descriptor(
                CompileMethodDescriptor {
                    id: definition.id,
                    owner: definition.owner,
                    member_name: definition.path.name.clone(),
                    debug_name: format!(
                        "{}::{}",
                        self.catalog
                            .ty(definition.owner)
                            .map(|ty| source_name(&ty.path))
                            .unwrap_or_else(|| format!("#{}", definition.owner.get())),
                        definition.path.name
                    ),
                    class,
                    signature,
                    access: CompileMethodAccess::new(
                        definition.access.public,
                        definition.access.reflect_callable,
                        definition.access.required_permissions().to_vec(),
                    ),
                },
                origin,
            )
            .map_err(input_error)
    }

    pub(super) fn host_type_target(&self, type_id: TypeId) -> Option<HostTypeTarget> {
        let definition = self.catalog.ty(type_id)?;
        Some(HostTypeTarget {
            semantic: type_id,
            runtime: host_type_id(definition.host_runtime_id?)?,
        })
    }
}

pub(super) fn unresolved_native(path: &str, span: vela_common::Span) -> CompileError {
    CompileError::new(CompileErrorKind::SemanticDiagnostics(vec![
        Diagnostic::error(format!("unresolved native function `{path}`"))
            .with_code("compiler::unresolved_native_function")
            .with_span(span)
            .with_label(span, "native function is not registered"),
    ]))
}

pub(super) fn unresolved_method(method: &str, span: vela_common::Span) -> CompileError {
    CompileError::new(CompileErrorKind::SemanticDiagnostics(vec![
        Diagnostic::error(format!("unresolved method `{method}`"))
            .with_code("compiler::unresolved_method")
            .with_span(span)
            .with_label(span, "method is not defined for the known receiver type"),
    ]))
}

pub(super) fn source_name(path: &DefPath) -> String {
    path.module
        .iter()
        .chain(std::iter::once(&path.name))
        .cloned()
        .collect::<Vec<_>>()
        .join("::")
}

pub(super) fn derived_native_function_id(name: &str) -> FunctionId {
    if let Some((module, function)) = name.rsplit_once("::")
        && let Some(id) = vela_stdlib::std_function_id(module, function)
    {
        return id;
    }
    let mut segments = name.split("::").collect::<Vec<_>>();
    let function = segments.pop().unwrap_or(name);
    FunctionId::from_def_id(DefPath::function("host", segments, function).id())
}

fn derived_external_type_path(name: &str) -> DefPath {
    let mut segments = name.split("::").collect::<Vec<_>>();
    let ty = segments.pop().unwrap_or(name);
    DefPath::ty("host", segments, ty)
}

pub(super) fn effect(value: vela_registry::EffectSet) -> MirEffect {
    MirEffect {
        may_trap: value != vela_registry::EffectSet::default(),
        may_allocate: value != vela_registry::EffectSet::default(),
        host_read: value.host_read,
        host_write: value.host_write,
        reflection_read: value.reflection_read,
        reflection_write: value.reflection_write,
        reflection_call: value.reflection_call,
        emits_event: value.event_emit,
        reads_time: value.time,
        uses_random: value.random,
        reads_io: value.io_read,
        writes_io: value.io_write,
        ..MirEffect::PURE
    }
}

pub(super) fn field_access(definition: &FieldDef) -> CompileFieldAccess {
    CompileFieldAccess::new(
        definition.access.readable,
        definition.access.writable,
        definition.access.reflect_readable,
        definition.access.reflect_writable,
        definition.access.required_permissions().to_vec(),
    )
}

pub(super) fn external_signature(
    signature: &vela_registry::FunctionSignature,
    effects: vela_registry::EffectSet,
    catalog: &ExternalCatalog,
    origin: MirSourceOrigin,
) -> CompileResult<CompileSignature> {
    Ok(CompileSignature {
        parameters: signature
            .params
            .iter()
            .map(|parameter| {
                Ok(CompileParameter {
                    name: parameter.name.clone(),
                    contract: parameter
                        .type_hint
                        .as_ref()
                        .map(|hint| external_hint_contract(hint, catalog, origin))
                        .transpose()?
                        .flatten(),
                    default: if parameter.has_default {
                        CompileParameterDefault::RuntimeProvided
                    } else {
                        CompileParameterDefault::Required
                    },
                    origin: None,
                })
            })
            .collect::<CompileResult<Vec<_>>>()?,
        // Frozen bytecode behavior permits omitted known prefixes and extra
        // positional arguments; only named calls are statically reordered.
        positional: CompilePositionalPolicy::RuntimeChecked,
        return_contract: signature
            .return_type
            .as_ref()
            .map(|hint| external_hint_contract(hint, catalog, origin))
            .transpose()?
            .flatten(),
        effect: effect(effects),
    })
}

fn external_hint_contract(
    hint: &TypeHintDef,
    catalog: &ExternalCatalog,
    origin: MirSourceOrigin,
) -> CompileResult<Option<MirTypeContract>> {
    if hint.path.as_slice() == ["Any"] && hint.args.is_empty() {
        return Ok(None);
    }
    validate_external_hint_names(hint, catalog, origin)?;
    super::schema::registry_hint_contract(hint, catalog)
        .and_then(super::schema::meaningful_contract)
        .map(Some)
        .ok_or_else(|| {
            CompileError::new(CompileErrorKind::RegistrySnapshot(format!(
                "unresolved external type hint `{hint}`"
            )))
            .with_span(origin.span)
        })
}

fn validate_external_hint_names(
    hint: &TypeHintDef,
    catalog: &ExternalCatalog,
    origin: MirSourceOrigin,
) -> CompileResult<()> {
    for argument in &hint.args {
        if argument.path.as_slice() != ["Any"] || !argument.args.is_empty() {
            validate_external_hint_names(argument, catalog, origin)?;
        }
    }
    let name = hint.path.join("::");
    let builtin = matches!(
        name.as_str(),
        "()" | "Any"
            | "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "f32"
            | "f64"
            | "char"
            | "String"
            | "Bytes"
            | "Array"
            | "Map"
            | "Set"
            | "Iterator"
            | "Option"
            | "Result"
            | "Range"
            | "Function"
            | "Closure"
    );
    if builtin || catalog.type_by_source(&name).is_some() {
        return Ok(());
    }
    Err(
        CompileError::new(CompileErrorKind::RegistrySnapshot(format!(
            "unresolved external type hint `{hint}`"
        )))
        .with_span(origin.span),
    )
}

fn insert_unambiguous<K: Ord, V: Copy + Eq>(map: &mut BTreeMap<K, Option<V>>, key: K, id: V) {
    match map.entry(key) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(Some(id));
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            if entry.get() != &Some(id) {
                entry.insert(None);
            }
        }
    }
}

fn host_type_id(value: u128) -> Option<HostTypeId> {
    u64::try_from(value).ok().map(HostTypeId::new)
}
