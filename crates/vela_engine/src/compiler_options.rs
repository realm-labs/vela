use std::collections::BTreeSet;

use vela_bytecode::compiler::options::{CompilerOptions, HostIndexCapabilityInfo};
use vela_reflect::registry::TypeRegistry;
use vela_registry::TypeHintDef;

use crate::native::{NativeFunctionDesc, TypeHint};

pub(crate) fn compiler_options_from_registry(registry: &TypeRegistry) -> CompilerOptions {
    let mut options = CompilerOptions::new();
    let registered_types = registry
        .types()
        .filter(|desc| {
            desc.host_type_id.is_some() || desc.kind != vela_reflect::registry::TypeKind::Host
        })
        .flat_map(|desc| {
            [
                desc.key.name.clone(),
                desc.key
                    .name
                    .rsplit("::")
                    .next()
                    .unwrap_or(&desc.key.name)
                    .to_owned(),
            ]
        })
        .collect::<BTreeSet<_>>();
    for module in registry.modules() {
        if let Some(root) = module.name.split("::").next() {
            options = options.with_native_module_root(root);
        }
    }
    for desc in registry.types() {
        for hint in desc
            .fields
            .iter()
            .filter_map(|field| field.type_hint.as_deref())
            .chain(
                desc.variants
                    .iter()
                    .flat_map(|variant| &variant.fields)
                    .filter_map(|field| field.type_hint.as_deref()),
            )
            .chain(
                desc.methods
                    .iter()
                    .flat_map(|method| &method.params)
                    .filter_map(|parameter| parameter.type_hint.as_deref()),
            )
            .chain(
                desc.methods
                    .iter()
                    .filter_map(|method| method.return_type.as_deref()),
            )
        {
            options = add_registry_type_hint(options, hint, &registered_types);
        }
        if let Some(index) = &desc.index_capability {
            for hint in [index.key_type.as_deref(), index.value_type.as_deref()]
                .into_iter()
                .flatten()
            {
                options = add_registry_type_hint(options, hint, &registered_types);
            }
            options = options.with_host_index_capability(
                desc.key.name.clone(),
                HostIndexCapabilityInfo {
                    readable: index.readable,
                    writable: index.writable,
                    addable: index.addable,
                    removable: index.removable,
                    key_type: index.key_type.clone(),
                    value_type: index.value_type.clone(),
                },
            );
        }
    }
    options
}

fn add_registry_type_hint(
    options: CompilerOptions,
    hint: &str,
    registered_types: &BTreeSet<String>,
) -> CompilerOptions {
    let Some(hint) = TypeHintDef::parse(hint) else {
        return options;
    };
    add_hint_def(options, &hint, registered_types)
}

fn add_hint_def(
    mut options: CompilerOptions,
    hint: &TypeHintDef,
    registered_types: &BTreeSet<String>,
) -> CompilerOptions {
    for argument in &hint.args {
        options = add_hint_def(options, argument, registered_types);
    }
    let name = hint.path.join("::");
    if !registered_types.contains(&name)
        && !matches!(
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
        )
    {
        options = options.with_opaque_external_type_hint(name);
    }
    options
}

pub(crate) fn add_native_signature_hints(
    mut options: CompilerOptions,
    desc: &NativeFunctionDesc,
) -> CompilerOptions {
    for hint in desc
        .params
        .iter()
        .map(|parameter| &parameter.hint)
        .chain(std::iter::once(&desc.returns))
    {
        options = add_type_hint(options, hint);
    }
    options
}

fn add_type_hint(mut options: CompilerOptions, hint: &TypeHint) -> CompilerOptions {
    match hint {
        TypeHint::ArrayOf(item)
        | TypeHint::ArrayViewOf(item)
        | TypeHint::SetOf(item)
        | TypeHint::SetViewOf(item)
        | TypeHint::IteratorOf(item)
        | TypeHint::OptionOf(item) => add_type_hint(options, item),
        TypeHint::ArrayMutOf { element, .. } | TypeHint::SetMutOf { element, .. } => {
            add_type_hint(options, element)
        }
        TypeHint::MapOf { key, value }
        | TypeHint::MapViewOf { key, value }
        | TypeHint::MapMutOf { key, value, .. }
        | TypeHint::ResultOf {
            ok: key,
            err: value,
        } => {
            options = add_type_hint(options, key);
            add_type_hint(options, value)
        }
        TypeHint::TupleOf(elements) => elements.iter().fold(options, add_type_hint),
        TypeHint::Record(key) | TypeHint::Enum(key) | TypeHint::Host(key) => {
            options.with_opaque_external_type_hint(key.name.clone())
        }
        TypeHint::Trait(name) => options.with_opaque_external_type_hint(name.clone()),
        TypeHint::Any
        | TypeHint::Primitive(_)
        | TypeHint::Array
        | TypeHint::Map
        | TypeHint::Set
        | TypeHint::Iterator
        | TypeHint::Function => options,
        TypeHint::PathProxy => options.with_opaque_external_type_hint("path_proxy"),
    }
}
