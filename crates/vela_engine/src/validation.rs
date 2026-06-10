use std::collections::BTreeSet;

use vela_reflect::modules::ModuleDesc;
use vela_reflect::registry::{AttrMap, MethodParamDesc, TypeDesc};

use crate::error::{EngineError, EngineErrorKind, EngineResult};
use crate::method::{NativeMethodDesc, NativeMethodEntry};
use crate::native::{
    ContextHostNativeFunctionEntry, HostNativeFunctionEntry, NativeFunctionDesc,
    NativeFunctionEntry, TypeHint,
};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ModuleValidationOptions {
    include_standard_modules: bool,
    include_time_module: bool,
    include_math_module: bool,
    include_io_module: bool,
    include_fs_module: bool,
}

impl ModuleValidationOptions {
    pub(crate) const fn include_standard_modules(mut self, include: bool) -> Self {
        self.include_standard_modules = include;
        self
    }

    pub(crate) const fn include_time_module(mut self, include: bool) -> Self {
        self.include_time_module = include;
        self
    }

    pub(crate) const fn include_math_module(mut self, include: bool) -> Self {
        self.include_math_module = include;
        self
    }

    pub(crate) const fn include_io_module(mut self, include: bool) -> Self {
        self.include_io_module = include;
        self
    }

    pub(crate) const fn include_fs_module(mut self, include: bool) -> Self {
        self.include_fs_module = include;
        self
    }
}

pub(crate) fn validate_modules(
    modules: &[ModuleDesc],
    options: ModuleValidationOptions,
) -> EngineResult<()> {
    let mut names = BTreeSet::new();
    if options.include_standard_modules {
        for module in crate::standard::standard_module_descs() {
            validate_module_desc(&module, &mut names)?;
        }
    }
    if options.include_time_module {
        validate_module_desc(&crate::clock::time_module_desc(), &mut names)?;
    }
    if options.include_math_module && !options.include_standard_modules {
        validate_module_desc(&ModuleDesc::new("math"), &mut names)?;
    }
    if options.include_io_module {
        validate_module_desc(&crate::io::io_module_desc(), &mut names)?;
    }
    if options.include_fs_module {
        validate_module_desc(&crate::io::fs_module_desc(), &mut names)?;
    }
    for module in modules {
        validate_module_desc(module, &mut names)?;
    }
    Ok(())
}

fn validate_module_desc(module: &ModuleDesc, names: &mut BTreeSet<String>) -> EngineResult<()> {
    if !is_valid_qualified_name(&module.name) {
        return Err(EngineError::new(EngineErrorKind::InvalidModuleName {
            name: module.name.clone(),
        }));
    }
    validate_attr_names(&format!("module {}", module.name), &module.attrs)?;
    if !names.insert(module.name.clone()) {
        return Err(EngineError::new(EngineErrorKind::DuplicateModuleName {
            name: module.name.clone(),
        }));
    }
    Ok(())
}

pub(crate) fn validate_types(types: &[TypeDesc], include_standard_types: bool) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut host_ids = BTreeSet::new();
    let mut host_method_ids = BTreeSet::new();
    let mut host_method_names = BTreeSet::new();

    if include_standard_types {
        for desc in crate::standard::standard_type_descs() {
            validate_type_desc(
                &desc,
                &mut ids,
                &mut names,
                &mut host_ids,
                &mut host_method_ids,
                &mut host_method_names,
            )?;
        }
    }

    for desc in types {
        validate_type_desc(
            desc,
            &mut ids,
            &mut names,
            &mut host_ids,
            &mut host_method_ids,
            &mut host_method_names,
        )?;
    }

    Ok(())
}

fn validate_type_desc(
    desc: &TypeDesc,
    ids: &mut BTreeSet<vela_def::TypeId>,
    names: &mut BTreeSet<String>,
    host_ids: &mut BTreeSet<vela_common::HostTypeId>,
    host_method_ids: &mut BTreeSet<vela_common::HostMethodId>,
    host_method_names: &mut BTreeSet<(String, String)>,
) -> EngineResult<()> {
    if !is_valid_simple_name(&desc.key.name) {
        return Err(EngineError::new(EngineErrorKind::InvalidTypeName {
            name: desc.key.name.clone(),
        }));
    }
    if !ids.insert(desc.key.id) {
        return Err(EngineError::new(EngineErrorKind::DuplicateTypeId {
            id: desc.key.id.get(),
        }));
    }
    if !names.insert(desc.key.name.clone()) {
        return Err(EngineError::new(EngineErrorKind::DuplicateTypeName {
            name: desc.key.name.clone(),
        }));
    }
    validate_attr_names(&format!("type {}", desc.key.name), &desc.attrs)?;
    if let Some(host_type_id) = desc.host_type_id
        && !host_ids.insert(host_type_id)
    {
        return Err(EngineError::new(EngineErrorKind::DuplicateHostTypeId {
            id: host_type_id.get(),
        }));
    }

    validate_type_fields(desc)?;
    validate_type_variants(desc)?;
    validate_type_traits(desc)?;
    validate_type_index_capability(desc)?;

    for method in &desc.methods {
        validate_schema_member_name(&desc.key.name, "host method", &method.name)?;
        validate_raw_type_hint(
            &format!("host method {}.{} return", desc.key.name, method.name),
            method.return_type.as_deref(),
        )?;
        validate_attr_names(
            &format!("host method {}.{}", desc.key.name, method.name),
            &method.attrs,
        )?;
        if !host_method_ids.insert(method.id) {
            return Err(EngineError::new(EngineErrorKind::DuplicateHostMethodId {
                id: method.id.get(),
            }));
        }
        if !host_method_names.insert((desc.key.name.clone(), method.name.clone())) {
            return Err(EngineError::new(EngineErrorKind::DuplicateHostMethodName {
                name: method.name.clone(),
            }));
        }
        validate_host_method_params(desc.key.name.as_str(), method.name.as_str(), &method.params)?;
        validate_permission_names(
            &format!("host method {}.{}", desc.key.name, method.name),
            method
                .access
                .required_permissions()
                .iter()
                .map(String::as_str),
        )?;
    }

    Ok(())
}

fn validate_type_index_capability(desc: &TypeDesc) -> EngineResult<()> {
    let Some(index) = &desc.index_capability else {
        return Ok(());
    };
    validate_raw_type_hint(
        &format!("index key type {}", desc.key.name),
        index.key_type.as_deref(),
    )?;
    validate_raw_type_hint(
        &format!("index value type {}", desc.key.name),
        index.value_type.as_deref(),
    )
}

fn validate_type_fields(desc: &TypeDesc) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for field in &desc.fields {
        validate_schema_member_name(&desc.key.name, "field", &field.name)?;
        validate_raw_type_hint(
            &format!("field {}.{}", desc.key.name, field.name),
            field.type_hint.as_deref(),
        )?;
        validate_attr_names(
            &format!("field {}.{}", desc.key.name, field.name),
            &field.attrs,
        )?;
        validate_permission_names(
            &format!("field {}.{}", desc.key.name, field.name),
            field
                .access
                .required_permissions()
                .iter()
                .map(String::as_str),
        )?;
        if !ids.insert(field.id) {
            return Err(EngineError::new(EngineErrorKind::DuplicateFieldId {
                type_name: desc.key.name.clone(),
                id: field.id.get(),
            }));
        }
        if !names.insert(field.name.as_str()) {
            return Err(EngineError::new(EngineErrorKind::DuplicateFieldName {
                type_name: desc.key.name.clone(),
                name: field.name.clone(),
            }));
        }
    }
    Ok(())
}

fn validate_type_variants(desc: &TypeDesc) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for variant in &desc.variants {
        validate_schema_member_name(&desc.key.name, "variant", &variant.name)?;
        validate_attr_names(
            &format!("variant {}::{}", desc.key.name, variant.name),
            &variant.attrs,
        )?;
        if !ids.insert(variant.id) {
            return Err(EngineError::new(EngineErrorKind::DuplicateVariantId {
                type_name: desc.key.name.clone(),
                id: variant.id.get(),
            }));
        }
        if !names.insert(variant.name.as_str()) {
            return Err(EngineError::new(EngineErrorKind::DuplicateVariantName {
                type_name: desc.key.name.clone(),
                name: variant.name.clone(),
            }));
        }
        validate_variant_fields(desc.key.name.as_str(), variant)?;
    }
    Ok(())
}

fn validate_variant_fields(
    type_name: &str,
    variant: &vela_reflect::registry::VariantDesc,
) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for field in &variant.fields {
        validate_schema_member_name(type_name, "variant field", &field.name)?;
        validate_raw_type_hint(
            &format!(
                "variant field {}::{}::{}",
                type_name, variant.name, field.name
            ),
            field.type_hint.as_deref(),
        )?;
        validate_attr_names(
            &format!(
                "variant field {}::{}::{}",
                type_name, variant.name, field.name
            ),
            &field.attrs,
        )?;
        validate_permission_names(
            &format!(
                "variant field {}::{}::{}",
                type_name, variant.name, field.name
            ),
            field
                .access
                .required_permissions()
                .iter()
                .map(String::as_str),
        )?;
        if !ids.insert(field.id) {
            return Err(EngineError::new(EngineErrorKind::DuplicateVariantFieldId {
                type_name: type_name.to_owned(),
                variant: variant.name.clone(),
                id: field.id.get(),
            }));
        }
        if !names.insert(field.name.as_str()) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateVariantFieldName {
                    type_name: type_name.to_owned(),
                    variant: variant.name.clone(),
                    name: field.name.clone(),
                },
            ));
        }
    }
    Ok(())
}

fn validate_type_traits(desc: &TypeDesc) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for trait_desc in &desc.traits {
        validate_schema_member_name(&desc.key.name, "trait", &trait_desc.name)?;
        validate_attr_names(
            &format!("trait {}::{}", desc.key.name, trait_desc.name),
            &trait_desc.attrs,
        )?;
        if !ids.insert(trait_desc.id) {
            return Err(EngineError::new(EngineErrorKind::DuplicateTraitId {
                type_name: desc.key.name.clone(),
                id: trait_desc.id.get(),
            }));
        }
        if !names.insert(trait_desc.name.as_str()) {
            return Err(EngineError::new(EngineErrorKind::DuplicateTraitName {
                type_name: desc.key.name.clone(),
                name: trait_desc.name.clone(),
            }));
        }
        validate_trait_methods(desc.key.name.as_str(), trait_desc)?;
    }
    Ok(())
}

fn validate_trait_methods(
    type_name: &str,
    trait_desc: &vela_reflect::registry::TraitDesc,
) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for method in &trait_desc.methods {
        validate_schema_member_name(type_name, "trait method", &method.name)?;
        validate_raw_type_hint(
            &format!(
                "trait method {}::{}::{} return",
                type_name, trait_desc.name, method.name
            ),
            method.return_type.as_deref(),
        )?;
        validate_attr_names(
            &format!(
                "trait method {}::{}::{}",
                type_name, trait_desc.name, method.name
            ),
            &method.attrs,
        )?;
        if !ids.insert(method.id) {
            return Err(EngineError::new(EngineErrorKind::DuplicateTraitMethodId {
                type_name: type_name.to_owned(),
                trait_name: trait_desc.name.clone(),
                id: method.id.get(),
            }));
        }
        if !names.insert(method.name.as_str()) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateTraitMethodName {
                    type_name: type_name.to_owned(),
                    trait_name: trait_desc.name.clone(),
                    name: method.name.clone(),
                },
            ));
        }
        validate_trait_method_params(
            type_name,
            trait_desc.name.as_str(),
            method.name.as_str(),
            &method.params,
        )?;
    }
    Ok(())
}

fn validate_host_method_params(
    type_name: &str,
    method: &str,
    params: &[MethodParamDesc],
) -> EngineResult<()> {
    let mut names = BTreeSet::new();
    for param in params {
        validate_schema_member_name(type_name, "host method parameter", &param.name)?;
        validate_raw_type_hint(
            &format!("host method {type_name}.{method} parameter {}", param.name),
            param.type_hint.as_deref(),
        )?;
        if !names.insert(param.name.as_str()) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateHostMethodParamName {
                    type_name: type_name.to_owned(),
                    method: method.to_owned(),
                    name: param.name.clone(),
                },
            ));
        }
    }
    Ok(())
}

fn validate_trait_method_params(
    type_name: &str,
    trait_name: &str,
    method: &str,
    params: &[MethodParamDesc],
) -> EngineResult<()> {
    let mut names = BTreeSet::new();
    for param in params {
        validate_schema_member_name(type_name, "trait method parameter", &param.name)?;
        validate_raw_type_hint(
            &format!(
                "trait method {type_name}::{trait_name}::{method} parameter {}",
                param.name
            ),
            param.type_hint.as_deref(),
        )?;
        if !names.insert(param.name.as_str()) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateTraitMethodParamName {
                    type_name: type_name.to_owned(),
                    trait_name: trait_name.to_owned(),
                    method: method.to_owned(),
                    name: param.name.clone(),
                },
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_native_functions(
    functions: &[NativeFunctionEntry],
    host_functions: &[HostNativeFunctionEntry],
    context_host_functions: &[ContextHostNativeFunctionEntry],
    types: &[TypeDesc],
    include_standard_natives: bool,
) -> EngineResult<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let type_hints = TypeHintLookup::new(types, include_standard_natives);

    if include_standard_natives {
        for desc in crate::standard::standard_native_function_descs() {
            validate_native_function_desc(&desc, &mut ids, &mut names, &type_hints)?;
        }
    }

    for desc in functions
        .iter()
        .map(|entry| &entry.desc)
        .chain(host_functions.iter().map(|entry| &entry.desc))
        .chain(context_host_functions.iter().map(|entry| &entry.desc))
    {
        validate_native_function_desc(desc, &mut ids, &mut names, &type_hints)?;
    }

    Ok(())
}

pub(crate) fn validate_native_method_type_hints(
    host_method_metadata: &[NativeMethodDesc],
    native_methods: &[NativeMethodEntry],
    types: &[TypeDesc],
    include_standard_types: bool,
) -> EngineResult<()> {
    let type_hints = TypeHintLookup::new(types, include_standard_types);
    for desc in host_method_metadata
        .iter()
        .chain(native_methods.iter().map(|entry| &entry.desc))
    {
        validate_attr_names(
            &format!("host method {}.{}", desc.owner.name, desc.name),
            &desc.attrs,
        )?;
        validate_type_hint(
            &desc.returns,
            &format!("host method {}.{} return", desc.owner.name, desc.name),
            &type_hints,
        )?;
        for param in &desc.params {
            validate_type_hint(
                &param.hint,
                &format!(
                    "host method {}.{} parameter {}",
                    desc.owner.name, desc.name, param.name
                ),
                &type_hints,
            )?;
        }
    }
    Ok(())
}

fn validate_native_function_desc(
    desc: &NativeFunctionDesc,
    ids: &mut BTreeSet<crate::native::NativeFunctionId>,
    names: &mut BTreeSet<String>,
    type_hints: &TypeHintLookup,
) -> EngineResult<()> {
    if !is_valid_qualified_name(&desc.name) {
        return Err(EngineError::new(
            EngineErrorKind::InvalidNativeFunctionName {
                name: desc.name.clone(),
            },
        ));
    }
    if !ids.insert(desc.id) {
        return Err(EngineError::new(
            EngineErrorKind::DuplicateNativeFunctionId { id: desc.id.get() },
        ));
    }
    if !names.insert(desc.name.clone()) {
        return Err(EngineError::new(
            EngineErrorKind::DuplicateNativeFunctionName {
                name: desc.name.clone(),
            },
        ));
    }
    validate_attr_names(&format!("native function {}", desc.name), &desc.attrs)?;
    validate_type_hint(
        &desc.returns,
        &format!("native function {} return", desc.name),
        type_hints,
    )?;
    validate_native_function_params(desc, type_hints)
}

fn validate_native_function_params(
    desc: &NativeFunctionDesc,
    type_hints: &TypeHintLookup,
) -> EngineResult<()> {
    let mut names = BTreeSet::new();
    for param in &desc.params {
        if !is_valid_simple_name(&param.name) {
            return Err(EngineError::new(
                EngineErrorKind::InvalidNativeFunctionParamName {
                    function: desc.name.clone(),
                    name: param.name.clone(),
                },
            ));
        }
        validate_type_hint(
            &param.hint,
            &format!("native function {} parameter {}", desc.name, param.name),
            type_hints,
        )?;
        if !names.insert(param.name.as_str()) {
            return Err(EngineError::new(
                EngineErrorKind::DuplicateNativeFunctionParamName {
                    function: desc.name.clone(),
                    name: param.name.clone(),
                },
            ));
        }
    }
    Ok(())
}

struct TypeHintLookup {
    types: BTreeSet<vela_reflect::registry::TypeKey>,
    traits: BTreeSet<String>,
}

impl TypeHintLookup {
    fn new(types: &[TypeDesc], include_standard_types: bool) -> Self {
        let mut lookup = Self {
            types: BTreeSet::new(),
            traits: BTreeSet::new(),
        };
        if include_standard_types {
            for desc in crate::standard::standard_type_descs() {
                lookup.insert(&desc);
            }
        }
        for desc in types {
            lookup.insert(desc);
        }
        lookup
    }

    fn insert(&mut self, desc: &TypeDesc) {
        self.types.insert(desc.key.clone());
        self.traits
            .extend(desc.traits.iter().map(|trait_desc| trait_desc.name.clone()));
    }
}

fn validate_type_hint(
    hint: &TypeHint,
    descriptor: &str,
    lookup: &TypeHintLookup,
) -> EngineResult<()> {
    match hint {
        TypeHint::Record(key) | TypeHint::Enum(key) => {
            if lookup.types.contains(key) {
                Ok(())
            } else {
                Err(EngineError::new(EngineErrorKind::UnknownTypeHint {
                    descriptor: descriptor.to_owned(),
                    type_name: key.name.clone(),
                }))
            }
        }
        TypeHint::Trait(name) => {
            if lookup.traits.contains(name) {
                Ok(())
            } else {
                Err(EngineError::new(EngineErrorKind::UnknownTypeHint {
                    descriptor: descriptor.to_owned(),
                    type_name: name.clone(),
                }))
            }
        }
        TypeHint::Any
        | TypeHint::Null
        | TypeHint::Bool
        | TypeHint::Int
        | TypeHint::Float
        | TypeHint::String
        | TypeHint::Array
        | TypeHint::Map
        | TypeHint::Set
        | TypeHint::PathProxy
        | TypeHint::Host(_)
        | TypeHint::Function => Ok(()),
    }
}

fn validate_permission_names<'a>(
    descriptor: &str,
    permissions: impl IntoIterator<Item = &'a str>,
) -> EngineResult<()> {
    for permission in permissions {
        if permission.is_empty() {
            return Err(EngineError::new(EngineErrorKind::InvalidPermissionName {
                descriptor: descriptor.to_owned(),
                name: permission.to_owned(),
            }));
        }
    }
    Ok(())
}

fn validate_attr_names(descriptor: &str, attrs: &AttrMap) -> EngineResult<()> {
    for (name, _) in attrs.iter() {
        if name.is_empty() {
            return Err(EngineError::new(EngineErrorKind::InvalidAttributeName {
                descriptor: descriptor.to_owned(),
                name: name.to_owned(),
            }));
        }
    }
    Ok(())
}

fn validate_raw_type_hint(descriptor: &str, hint: Option<&str>) -> EngineResult<()> {
    let Some(hint) = hint else {
        return Ok(());
    };
    if hint.is_empty()
        || hint.trim() != hint
        || hint.contains('<')
        || hint.contains('>')
        || !is_valid_qualified_name(hint)
    {
        return Err(EngineError::new(EngineErrorKind::InvalidTypeHintName {
            descriptor: descriptor.to_owned(),
            type_name: hint.to_owned(),
        }));
    }
    Ok(())
}

fn is_valid_qualified_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('.') && name.split("::").all(|segment| !segment.is_empty())
}

fn validate_schema_member_name(type_name: &str, member_kind: &str, name: &str) -> EngineResult<()> {
    if is_valid_simple_name(name) {
        return Ok(());
    }
    Err(EngineError::new(EngineErrorKind::InvalidSchemaMemberName {
        type_name: type_name.to_owned(),
        member_kind: member_kind.to_owned(),
        name: name.to_owned(),
    }))
}

fn is_valid_simple_name(name: &str) -> bool {
    !name.is_empty()
}
