use crate::type_binding::TypeBindingRegistry;
use vela_def::{DefPath, VariantId};
use vela_reflect::registry::{
    FieldDesc, HostIndexCapability, MethodDesc, MethodParamDesc, TypeDesc, TypeKind, VariantDesc,
};
use vela_registry::{
    DefinitionRegistry, EffectSet as DefinitionEffectSet, FieldAccessDef, FieldDef,
    FunctionAccessDef, FunctionDef, FunctionSignature, IndexCapabilityDef, MethodAccessDef,
    MethodDef, ParamDef, RegistryError, TypeBindingDef, TypeDef, TypeHintDef, TypeKindDef,
    VariantDef,
};

use crate::native::{
    AsyncContextHostNativeFunctionEntry, AsyncDirectHostNativeFunctionEntry,
    AsyncHostNativeFunctionEntry, AsyncNativeFunctionEntry, ContextHostNativeFunctionEntry,
    HostNativeFunctionEntry, NativeFunctionDesc, NativeFunctionEntry,
    ScopedHostNativeFunctionEntry,
};

pub(crate) struct EngineFunctionEntries<'a> {
    pub(crate) native: &'a [NativeFunctionEntry],
    pub(crate) async_native: &'a [AsyncNativeFunctionEntry],
    pub(crate) async_host: &'a [AsyncHostNativeFunctionEntry],
    pub(crate) async_direct_host: &'a [AsyncDirectHostNativeFunctionEntry],
    pub(crate) scoped_host: &'a [ScopedHostNativeFunctionEntry],
    pub(crate) async_context_host: &'a [AsyncContextHostNativeFunctionEntry],
    pub(crate) host: &'a [HostNativeFunctionEntry],
    pub(crate) context_host: &'a [ContextHostNativeFunctionEntry],
}

pub(crate) fn definition_registry_from_engine_parts(
    types: &[TypeDesc],
    type_bindings: &TypeBindingRegistry,
    functions: EngineFunctionEntries<'_>,
    include_reflection_natives: bool,
    include_stdlib: bool,
) -> Result<DefinitionRegistry, RegistryError> {
    let mut registry = DefinitionRegistry::new();
    if include_stdlib {
        vela_stdlib::register_stdlib(&mut registry)?;
    }
    for desc in types {
        register_type_def(&mut registry, desc, type_bindings)?;
    }
    for desc in functions
        .native
        .iter()
        .map(|entry| &entry.desc)
        .chain(functions.async_native.iter().map(|entry| &entry.desc))
        .chain(functions.async_host.iter().map(|entry| &entry.desc))
        .chain(functions.async_direct_host.iter().map(|entry| &entry.desc))
        .chain(functions.scoped_host.iter().map(|entry| &entry.desc))
        .chain(functions.async_context_host.iter().map(|entry| &entry.desc))
        .chain(functions.host.iter().map(|entry| &entry.desc))
        .chain(functions.context_host.iter().map(|entry| &entry.desc))
    {
        registry.register_function(native_function_def(desc))?;
    }
    if include_reflection_natives {
        vela_stdlib::register_reflection_natives(&mut registry)?;
    }
    registry.seal_type_bindings(type_bindings.checksum());
    Ok(registry)
}

fn register_type_def(
    registry: &mut DefinitionRegistry,
    desc: &TypeDesc,
    type_bindings: &TypeBindingRegistry,
) -> Result<(), RegistryError> {
    let type_id = registry.register_type(type_def(desc, type_bindings))?;
    for (order, field) in desc.fields.iter().enumerate() {
        registry.register_field(field_def(desc, type_id, field, declaration_order(order)))?;
    }
    for (order, variant) in desc.variants.iter().enumerate() {
        let variant_id = registry.register_variant(variant_def(
            desc,
            type_id,
            variant,
            declaration_order(order),
        ))?;
        for (field_order, field) in variant.fields.iter().enumerate() {
            registry.register_field(variant_field_def(
                desc,
                type_id,
                variant_id,
                &variant.name,
                field,
                declaration_order(field_order),
            ))?;
        }
    }
    for method in &desc.methods {
        registry.register_method(method_def(desc, type_id, method))?;
    }
    Ok(())
}

fn type_def(desc: &TypeDesc, type_bindings: &TypeBindingRegistry) -> TypeDef {
    let mut def = TypeDef::new(source_type_path("host", &desc.key.name))
        .with_id(desc.key.id)
        .kind(definition_type_kind(desc.kind));
    if let Some(host_type_id) = desc.host_type_id {
        def = def.host_runtime_id(host_type_id.get().into());
    }
    if let Some(capability) = &desc.index_capability {
        def = def.index_capability(index_capability_def(capability));
    }
    if let Some(binding) = type_bindings.get(vela_common::InteropTypeId::from_type_id(desc.key.id))
    {
        def = def.binding(TypeBindingDef::new(
            binding.id,
            binding.storage,
            binding.capabilities,
            binding.abi_fingerprint,
        ));
    }
    def
}

fn index_capability_def(capability: &HostIndexCapability) -> IndexCapabilityDef {
    let mut definition = IndexCapabilityDef::new()
        .readable(capability.readable)
        .writable(capability.writable)
        .addable(capability.addable)
        .removable(capability.removable);
    if let Some(key_type) = capability.key_type.as_deref() {
        definition = definition.key_type(raw_type_hint_def(key_type));
    }
    if let Some(value_type) = capability.value_type.as_deref() {
        definition = definition.value_type(raw_type_hint_def(value_type));
    }
    definition
}

fn field_def(
    desc: &TypeDesc,
    owner: vela_def::TypeId,
    field: &FieldDesc,
    declaration_order: u32,
) -> FieldDef {
    FieldDef::new(
        source_field_path("host", &desc.key.name, &field.name),
        owner,
    )
    .host_runtime_id(field.id.get())
    .declaration_order(declaration_order)
    .defaulted(field.has_default)
    .access(field_access(&field.access))
    .type_hint(field.type_hint.as_deref().map(raw_type_hint_def))
}

fn variant_def(
    desc: &TypeDesc,
    owner: vela_def::TypeId,
    variant: &VariantDesc,
    declaration_order: u32,
) -> VariantDef {
    VariantDef::new(
        source_variant_path("host", &desc.key.name, &variant.name),
        owner,
    )
    .declaration_order(declaration_order)
}

fn variant_field_def(
    desc: &TypeDesc,
    owner: vela_def::TypeId,
    variant_id: VariantId,
    variant: &str,
    field: &FieldDesc,
    declaration_order: u32,
) -> FieldDef {
    FieldDef::new(
        source_field_path(
            "host",
            &format!("{}::{variant}", desc.key.name),
            &field.name,
        ),
        owner,
    )
    .host_runtime_id(field.id.get())
    .variant_owner(variant_id)
    .declaration_order(declaration_order)
    .defaulted(field.has_default)
    .access(field_access(&field.access))
    .type_hint(field.type_hint.as_deref().map(raw_type_hint_def))
}

fn method_def(desc: &TypeDesc, owner: vela_def::TypeId, method: &MethodDesc) -> MethodDef {
    MethodDef::new(
        source_method_path("host", &desc.key.name, &method.name),
        owner,
        FunctionSignature::new(
            method.params.iter().map(method_param_def),
            method.return_type.as_deref().map(raw_type_hint_def),
        )
        .asyncness(method.asyncness),
    )
    .host_runtime_id(method.id.get())
    .effects(method_effects(&method.effects))
    .access(method_access(&method.access))
}

fn method_param_def(param: &MethodParamDesc) -> ParamDef {
    ParamDef::new(
        param.name.clone(),
        param.type_hint.as_deref().map(raw_type_hint_def),
    )
    .defaulted(param.has_default)
}

fn native_function_def(desc: &NativeFunctionDesc) -> FunctionDef {
    let package = if desc.attrs.get("stdlib").is_some() {
        "std"
    } else {
        "host"
    };
    FunctionDef::new(
        source_function_path(package, &desc.name),
        FunctionSignature::new(
            desc.params
                .iter()
                .map(|param| ParamDef::new(param.name.clone(), Some(type_hint_def(&param.hint)))),
            Some(type_hint_def(&desc.returns)),
        )
        .asyncness(desc.asyncness),
    )
    .with_id(desc.id)
    .effects(native_function_effects(&desc.effects))
    .access(function_access(&desc.access))
}

fn source_function_path(package: &str, name: &str) -> DefPath {
    let mut parts = name
        .split("::")
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let function_name = parts.pop().unwrap_or(name);
    DefPath::function(package, parts, function_name)
}

fn source_type_path(package: &str, name: &str) -> DefPath {
    let mut parts = name
        .split("::")
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let type_name = parts.pop().unwrap_or(name);
    DefPath::ty(package, parts, type_name)
}

fn source_field_path(package: &str, owner: &str, name: &str) -> DefPath {
    DefPath::field(package, std::iter::empty::<&str>(), owner, name)
}

fn source_variant_path(package: &str, owner: &str, name: &str) -> DefPath {
    DefPath::variant(package, std::iter::empty::<&str>(), owner, name)
}

fn source_method_path(package: &str, owner: &str, name: &str) -> DefPath {
    DefPath::method(package, std::iter::empty::<&str>(), owner, name)
}

const fn definition_type_kind(kind: TypeKind) -> TypeKindDef {
    match kind {
        TypeKind::Unit => TypeKindDef::Unit,
        TypeKind::Bool => TypeKindDef::Bool,
        TypeKind::I8 => TypeKindDef::I8,
        TypeKind::I16 => TypeKindDef::I16,
        TypeKind::I32 => TypeKindDef::I32,
        TypeKind::I64 => TypeKindDef::I64,
        TypeKind::U8 => TypeKindDef::U8,
        TypeKind::U16 => TypeKindDef::U16,
        TypeKind::U32 => TypeKindDef::U32,
        TypeKind::U64 => TypeKindDef::U64,
        TypeKind::F32 => TypeKindDef::F32,
        TypeKind::F64 => TypeKindDef::F64,
        TypeKind::Char => TypeKindDef::Char,
        TypeKind::String => TypeKindDef::String,
        TypeKind::Bytes => TypeKindDef::Bytes,
        TypeKind::Array => TypeKindDef::Array,
        TypeKind::Map => TypeKindDef::Map,
        TypeKind::Set => TypeKindDef::Set,
        TypeKind::Range => TypeKindDef::Range,
        TypeKind::Function => TypeKindDef::Function,
        TypeKind::Closure => TypeKindDef::Closure,
        TypeKind::Host => TypeKindDef::Host,
        TypeKind::ScriptStruct => TypeKindDef::ScriptStruct,
        TypeKind::ScriptEnum => TypeKindDef::ScriptEnum,
    }
}

fn field_access(access: &vela_reflect::access::FieldAccess) -> FieldAccessDef {
    let mut definition = FieldAccessDef::new()
        .readable(access.readable)
        .writable(access.writable)
        .reflect_readable(access.reflect_readable)
        .reflect_writable(access.reflect_writable);
    for permission in access.required_permissions() {
        definition = definition.require_permission(permission);
    }
    definition
}

fn method_access(access: &vela_reflect::access::MethodAccess) -> MethodAccessDef {
    let mut definition = MethodAccessDef::new()
        .public(access.public)
        .reflect_callable(access.reflect_callable);
    for permission in access.required_permissions() {
        definition = definition.require_permission(permission);
    }
    definition
}

fn function_access(access: &crate::native::FunctionAccess) -> FunctionAccessDef {
    FunctionAccessDef::new()
        .public(access.public)
        .reflect_visible(access.reflect_visible)
        .reflect_callable(access.reflect_callable)
}

fn declaration_order(index: usize) -> u32 {
    u32::try_from(index).expect("definition declaration order exceeds u32::MAX")
}

fn native_function_effects(effects: &crate::native::EffectSet) -> DefinitionEffectSet {
    DefinitionEffectSet {
        host_read: effects.reads_host(),
        host_write: effects.writes_host(),
        reflection_read: effects.reads_reflection(),
        reflection_write: effects.writes_reflection(),
        reflection_call: effects.calls_reflection(),
        event_emit: effects.emits_events(),
        time: effects.reads_time(),
        random: effects.uses_random(),
        io_read: effects.reads_io(),
        io_write: effects.writes_io(),
    }
}

fn method_effects(effects: &vela_reflect::access::MethodEffectSet) -> DefinitionEffectSet {
    DefinitionEffectSet {
        host_read: effects.reads_host,
        host_write: effects.writes_host,
        reflection_read: effects.reads_reflection,
        reflection_write: effects.writes_reflection,
        reflection_call: effects.calls_reflection,
        event_emit: effects.emits_events,
        time: effects.reads_time,
        random: effects.uses_random,
        io_read: effects.reads_io,
        io_write: effects.writes_io,
    }
}

fn raw_type_hint_def(hint: &str) -> TypeHintDef {
    TypeHintDef::parse(hint).unwrap_or_else(|| TypeHintDef::named(hint))
}

fn type_hint_def(hint: &crate::native::TypeHint) -> TypeHintDef {
    match hint {
        crate::native::TypeHint::Any => TypeHintDef::named("Any"),
        crate::native::TypeHint::Primitive(vela_common::PrimitiveTag::String) => {
            TypeHintDef::named("String")
        }
        crate::native::TypeHint::Primitive(vela_common::PrimitiveTag::Bytes) => {
            TypeHintDef::named("Bytes")
        }
        crate::native::TypeHint::Primitive(tag) => TypeHintDef::named(tag.name()),
        crate::native::TypeHint::Array => TypeHintDef::named("Array"),
        crate::native::TypeHint::ArrayOf(element) => {
            TypeHintDef::named("Array").with_args([type_hint_def(element)])
        }
        crate::native::TypeHint::Map => TypeHintDef::named("Map"),
        crate::native::TypeHint::MapOf { key, value } => {
            TypeHintDef::named("Map").with_args([type_hint_def(key), type_hint_def(value)])
        }
        crate::native::TypeHint::Set => TypeHintDef::named("Set"),
        crate::native::TypeHint::SetOf(element) => {
            TypeHintDef::named("Set").with_args([type_hint_def(element)])
        }
        crate::native::TypeHint::TupleOf(elements) => {
            TypeHintDef::tuple(elements.iter().map(type_hint_def))
        }
        crate::native::TypeHint::Iterator => TypeHintDef::named("Iterator"),
        crate::native::TypeHint::IteratorOf(item) => {
            TypeHintDef::named("Iterator").with_args([type_hint_def(item)])
        }
        crate::native::TypeHint::OptionOf(payload) => {
            TypeHintDef::named("Option").with_args([type_hint_def(payload)])
        }
        crate::native::TypeHint::ResultOf { ok, err } => {
            TypeHintDef::named("Result").with_args([type_hint_def(ok), type_hint_def(err)])
        }
        crate::native::TypeHint::PathProxy => TypeHintDef::named("path_proxy"),
        crate::native::TypeHint::Record(key)
        | crate::native::TypeHint::Enum(key)
        | crate::native::TypeHint::Host(key) => TypeHintDef::named(key.name.clone()),
        crate::native::TypeHint::Trait(name) => TypeHintDef::named(name.clone()),
        crate::native::TypeHint::Function => TypeHintDef::named("Function"),
    }
}

#[cfg(test)]
mod tests {
    use vela_analysis::facts::AnalysisFacts;
    use vela_analysis::registry::RegistryFacts;
    use vela_analysis::semantic_facts::HostPathSegmentFact;
    use vela_analysis::type_fact::TypeFact;
    use vela_common::{HostMethodId, HostTypeId, SourceId};
    use vela_def::{FieldId, FunctionId, TypeId, VariantId};
    use vela_hir::body::HirExprKind;
    use vela_hir::module_graph::{ModuleGraph, ModuleSource};
    use vela_package::ModulePath;
    use vela_reflect::access::{FieldAccess, MethodAccess, MethodEffectSet};
    use vela_reflect::registry::{
        FieldDesc, HostIndexCapability, MethodDesc, TypeDesc, TypeKey, TypeKind, VariantDesc,
    };
    use vela_registry::{Def, IndexCapabilityDef, TypeHintDef, TypeKindDef};

    use super::{EngineFunctionEntries, definition_registry_from_engine_parts};
    use crate::native::{EffectSet, NativeFunctionDesc, NativeFunctionEntry};

    #[test]
    fn production_registry_conversion_preserves_compile_and_analysis_metadata() {
        let type_desc = TypeDesc::new(TypeKey::new(TypeId::new(70), "QuestState"))
            .kind(TypeKind::ScriptEnum)
            .host_type(HostTypeId::new(71))
            .field(
                FieldDesc::new(FieldId::new(72), "revision")
                    .type_hint("u32")
                    .defaulted(true)
                    .access(
                        FieldAccess::new()
                            .readable(false)
                            .writable(true)
                            .reflect_readable(true)
                            .reflect_writable(false)
                            .require_permission("quest.inspect"),
                    ),
            )
            .variant(
                VariantDesc::new(VariantId::new(73), "Active")
                    .field(FieldDesc::new(FieldId::new(74), "payload").type_hint("String"))
                    .field(
                        FieldDesc::new(FieldId::new(75), "count")
                            .type_hint("i64")
                            .defaulted(true),
                    ),
            )
            .variant(
                VariantDesc::new(VariantId::new(76), "Done")
                    .field(FieldDesc::new(FieldId::new(77), "payload").type_hint("String")),
            )
            .method(
                MethodDesc::new(HostMethodId::new(78), "rewrite")
                    .effects(MethodEffectSet {
                        writes_reflection: true,
                        ..MethodEffectSet::default()
                    })
                    .access(
                        MethodAccess::new()
                            .public(false)
                            .reflect_callable(false)
                            .require_permission("quest.admin"),
                    ),
            );
        let index_desc = TypeDesc::new(TypeKey::new(TypeId::new(80), "QuestIndex"))
            .kind(TypeKind::Host)
            .host_type(HostTypeId::new(81))
            .index_capability(
                HostIndexCapability::new()
                    .readable(true)
                    .writable(false)
                    .addable(true)
                    .removable(false)
                    .key_type("String")
                    .value_type("i64"),
            );
        let native = NativeFunctionEntry::new(
            NativeFunctionDesc::new("admin::rewrite", FunctionId::new(79))
                .effects(EffectSet::reflection_write())
                .access(
                    crate::native::FunctionAccess::private()
                        .reflect_visible(true)
                        .reflect_callable(true),
                ),
            |_| unreachable!("metadata-only native should not execute"),
        );

        let type_bindings = crate::type_binding::TypeBindingRegistry::seal(Vec::new(), &[])
            .expect("empty type binding registry");
        let registry = definition_registry_from_engine_parts(
            &[type_desc, index_desc],
            &type_bindings,
            EngineFunctionEntries {
                native: &[native],
                async_native: &[],
                async_host: &[],
                async_direct_host: &[],
                scoped_host: &[],
                async_context_host: &[],
                host: &[],
                context_host: &[],
            },
            true,
            false,
        )
        .expect("production registry conversion should succeed");
        let view = registry.compile_view();
        let definitions = view.definitions().collect::<Vec<_>>();
        let type_definition = definitions
            .iter()
            .find_map(|definition| match definition {
                Def::Type(definition) if definition.path.name == "QuestState" => Some(definition),
                _ => None,
            })
            .expect("QuestState type definition");
        assert_eq!(type_definition.kind, TypeKindDef::ScriptEnum);
        assert_eq!(type_definition.host_runtime_id, Some(71));
        assert!(type_definition.index_capability.is_none());
        let index_definition = definitions
            .iter()
            .find_map(|definition| match definition {
                Def::Type(definition) if definition.path.name == "QuestIndex" => Some(definition),
                _ => None,
            })
            .expect("QuestIndex type definition");
        assert_eq!(index_definition.kind, TypeKindDef::Host);
        assert_eq!(index_definition.host_runtime_id, Some(81));
        assert_eq!(
            index_definition.index_capability,
            Some(
                IndexCapabilityDef::new()
                    .readable(true)
                    .writable(false)
                    .addable(true)
                    .removable(false)
                    .key_type(TypeHintDef::named("String"))
                    .value_type(TypeHintDef::named("i64"))
            )
        );

        let mut variants = definitions
            .iter()
            .filter_map(|definition| match definition {
                Def::Variant(variant) if variant.owner == type_definition.id => Some(variant),
                _ => None,
            })
            .collect::<Vec<_>>();
        variants.sort_by_key(|variant| variant.declaration_order);
        assert_eq!(
            variants
                .iter()
                .map(|variant| (variant.path.name.as_str(), variant.declaration_order))
                .collect::<Vec<_>>(),
            [("Active", 0), ("Done", 1)]
        );

        let active = variants[0];
        let done = variants[1];
        let mut active_fields = definitions
            .iter()
            .filter_map(|definition| match definition {
                Def::Field(field) if field.variant == Some(active.id) => Some(field),
                _ => None,
            })
            .collect::<Vec<_>>();
        active_fields.sort_by_key(|field| field.declaration_order);
        assert_eq!(
            active_fields
                .iter()
                .map(|field| {
                    (
                        field.path.name.as_str(),
                        field.declaration_order,
                        field.has_default,
                    )
                })
                .collect::<Vec<_>>(),
            [("payload", 0, false), ("count", 1, true)]
        );
        assert!(definitions.iter().any(|definition| {
            matches!(definition, Def::Field(field)
                if field.variant == Some(done.id) && field.path.name == "payload")
        }));

        let root_field = definitions
            .iter()
            .find_map(|definition| match definition {
                Def::Field(field) if field.path.name == "revision" => Some(field),
                _ => None,
            })
            .expect("root field definition");
        assert!(!root_field.access.readable);
        assert!(root_field.access.writable);
        assert!(root_field.access.reflect_readable);
        assert!(!root_field.access.reflect_writable);
        assert_eq!(root_field.access.required_permissions(), ["quest.inspect"]);
        assert_eq!(root_field.declaration_order, 0);
        assert!(root_field.has_default);

        let method = definitions
            .iter()
            .find_map(|definition| match definition {
                Def::Method(method) if method.path.name == "rewrite" => Some(method),
                _ => None,
            })
            .expect("method definition");
        assert!(!method.access.public);
        assert!(!method.access.reflect_callable);
        assert_eq!(method.access.required_permissions(), ["quest.admin"]);
        assert!(method.effects.reflection_write);
        let native_function = definitions
            .iter()
            .find_map(|definition| match definition {
                Def::Function(function) if function.path.name == "rewrite" => Some(function),
                _ => None,
            })
            .expect("admin rewrite function definition");
        assert!(!native_function.access.public);
        assert!(native_function.access.reflect_visible);
        assert!(native_function.access.reflect_callable);

        let facts = RegistryFacts::from_compile_view(view).expect("registry declaration slots");
        let type_target = facts
            .type_target_fact("QuestState")
            .expect("semantic type target");
        assert_eq!(type_target.semantic, type_definition.id);
        assert_eq!(type_target.host_runtime, Some(HostTypeId::new(71)));
        let index_type_target = facts
            .type_target_fact("QuestIndex")
            .expect("semantic index type target");
        assert_eq!(index_type_target.semantic, index_definition.id);
        assert_eq!(index_type_target.host_runtime, Some(HostTypeId::new(81)));
        let index_capability = facts
            .index_capability_fact("QuestIndex")
            .expect("host index capability fact");
        assert!(index_capability.readable);
        assert!(!index_capability.writable);
        assert!(index_capability.addable);
        assert!(!index_capability.removable);
        assert_eq!(index_capability.key, TypeFact::STRING);
        assert_eq!(index_capability.value, TypeFact::I64);
        assert!(facts.variant_fact("QuestState", "Active").is_some());
        assert!(facts.variant_fact("QuestState", "Done").is_some());
        assert!(facts.field_fact("QuestState::Active", "payload").is_some());
        assert!(facts.field_fact("QuestState::Done", "payload").is_some());
        let field_target = facts
            .field_target_fact("QuestState", "revision")
            .expect("semantic field target");
        assert_eq!(field_target.semantic, root_field.id);
        assert_eq!(field_target.host_runtime, Some(FieldId::new(72)));
        assert_eq!(field_target.declaration_order, 0);
        assert!(field_target.has_default);
        assert_eq!(
            &field_target.access,
            facts
                .field_access_fact("QuestState", "revision")
                .expect("field access fact")
        );
        assert!(
            facts
                .method_access_fact("QuestState", "rewrite")
                .is_some_and(|access| {
                    !access.public
                        && !access.reflect_callable
                        && access.required_permissions == ["quest.admin"]
                })
        );
        assert!(
            facts
                .method_effect_fact("QuestState", "rewrite")
                .is_some_and(|effect| effect.writes_reflection)
        );
        assert!(
            facts
                .function_effect_fact("admin::rewrite")
                .is_some_and(|effect| effect.writes_reflection)
        );
        assert!(
            facts
                .function_access_fact("admin::rewrite")
                .is_some_and(|access| {
                    !access.public && access.reflect_visible && access.reflect_callable
                })
        );
        assert!(
            facts
                .function_effect_fact("reflect::set")
                .is_some_and(|effect| effect.writes_reflection)
        );
        assert!(
            facts
                .function_access_fact("reflect::set")
                .is_some_and(|access| {
                    access.public && access.reflect_visible && !access.reflect_callable
                })
        );

        let mut graph = ModuleGraph::new();
        graph.add_source(ModuleSource::new(
            SourceId::new(1),
            vela_package::PackageId::anonymous(),
            ModulePath::from_qualified("game"),
            "fn lookup(state: QuestIndex, key: i64) -> i64 { return state[key]; }",
        ));
        graph.resolve_imports();
        assert_eq!(graph.diagnostics(), &[]);
        let analysis = AnalysisFacts::from_module_graph_and_schema(&graph, &facts);
        let body = graph.bodies().next().expect("lookup body");
        let indexed = body
            .expressions
            .values()
            .find(|expression| matches!(expression.kind, HirExprKind::Index(_)))
            .expect("indexed host expression");
        let HirExprKind::Index(index) = &indexed.kind else {
            unreachable!("index expression selected above")
        };
        assert_eq!(analysis.expression(index.index), Some(&TypeFact::I64));
        let path = analysis
            .host_path_target(indexed.id)
            .expect("host index path target");
        let Some(HostPathSegmentFact::Index { capability, .. }) = path.segments.last() else {
            panic!("expected terminal host index segment: {path:?}");
        };
        assert_eq!(capability.key, TypeFact::STRING);
        assert_ne!(analysis.expression(index.index), Some(&capability.key));
    }
}
