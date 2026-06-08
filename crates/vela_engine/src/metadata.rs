use vela_reflect::access::{
    FunctionAccess as ReflectFunctionAccess, FunctionEffectSet, MethodAccess, MethodEffectSet,
};
use vela_reflect::modules::{DeclOrigin, FunctionDesc, FunctionParamDesc, ModuleDesc};
use vela_reflect::registry::{MethodDesc, MethodParamDesc, TypeDesc, TypeKey, TypeRegistry};

use crate::error::{EngineError, EngineErrorKind, EngineResult};
use crate::method::{NativeMethodDesc, NativeMethodEntry};
use crate::native::{
    ContextHostNativeFunctionEntry, HostNativeFunctionEntry, NativeFunctionDesc,
    NativeFunctionEntry, TypeHint,
};

pub(crate) fn inject_host_method_metadata(
    types: &mut [TypeDesc],
    host_method_metadata: &[NativeMethodDesc],
    native_methods: &[NativeMethodEntry],
) -> EngineResult<()> {
    for desc in host_method_metadata
        .iter()
        .chain(native_methods.iter().map(|entry| &entry.desc))
    {
        let owner = find_type_mut(types, &desc.owner).ok_or_else(|| {
            EngineError::new(EngineErrorKind::UnknownNativeMethodOwner {
                name: desc.owner.name.clone(),
            })
        })?;
        let mut method = MethodDesc::new(desc.id, desc.name.clone())
            .return_type(type_hint_display(&desc.returns))
            .effects(reflect_effects(&desc.effects))
            .access(reflect_access(&desc.access));
        for param in &desc.params {
            method = method.param(
                MethodParamDesc::new(param.name.clone()).type_hint(type_hint_display(&param.hint)),
            );
        }
        if let Some(docs) = &desc.docs {
            method = method.docs(docs.clone());
        }
        for (name, value) in desc.attrs.iter() {
            method = method.attr(name, value);
        }
        if let Some(source_span) = desc.source_span {
            method = method.source_span(source_span);
        }
        owner.methods.push(method);
    }
    Ok(())
}

pub(crate) fn inject_native_function_metadata(
    registry: &mut TypeRegistry,
    native_functions: &[NativeFunctionEntry],
    host_native_functions: &[HostNativeFunctionEntry],
    context_host_native_functions: &[ContextHostNativeFunctionEntry],
) {
    for desc in native_functions
        .iter()
        .map(|entry| &entry.desc)
        .chain(host_native_functions.iter().map(|entry| &entry.desc))
        .chain(
            context_host_native_functions
                .iter()
                .map(|entry| &entry.desc),
        )
    {
        if let Some(module_name) = native_function_module(&desc.name)
            && registry.module_by_name(&module_name).is_none()
        {
            registry.register_module(native_module_desc(&module_name));
        }
        registry.register_function(reflect_function(desc));
    }
}

pub(crate) fn inject_standard_native_metadata(registry: &mut TypeRegistry) {
    for desc in crate::standard::standard_module_descs() {
        if registry.module_by_name(&desc.name).is_none() {
            registry.register_module(desc);
        }
    }
    for desc in crate::standard::standard_type_descs() {
        if registry.type_by_name(&desc.key.name).is_some() {
            continue;
        }
        registry.register(desc);
    }
    for desc in crate::standard::standard_native_function_descs() {
        if registry.function_by_name(&desc.name).is_some() {
            continue;
        }
        if let Some(module_name) = native_function_module(&desc.name)
            && registry.module_by_name(&module_name).is_none()
        {
            registry.register_module(ModuleDesc::new(module_name));
        }
        registry.register_function(reflect_function(&desc));
    }
}

fn native_module_desc(module_name: &str) -> ModuleDesc {
    match module_name {
        "time" => crate::clock::time_module_desc(),
        "io" => crate::io::io_module_desc(),
        "fs" => crate::io::fs_module_desc(),
        _ => ModuleDesc::new(module_name),
    }
}

fn reflect_function(desc: &NativeFunctionDesc) -> FunctionDesc {
    let mut reflected = FunctionDesc::new(desc.id, desc.name.clone())
        .origin(DeclOrigin::Host)
        .return_type(type_hint_display(&desc.returns))
        .effects(reflect_function_effects(&desc.effects))
        .access(reflect_function_access(&desc.access));
    if let Some(module_name) = native_function_module(&desc.name) {
        reflected = reflected.module(module_name);
    }
    for param in &desc.params {
        reflected = reflected.param(
            FunctionParamDesc::new(param.name.clone()).type_hint(type_hint_display(&param.hint)),
        );
    }
    if let Some(docs) = &desc.docs {
        reflected = reflected.docs(docs.clone());
    }
    for (name, value) in desc.attrs.iter() {
        reflected = reflected.attr(name, value);
    }
    if let Some(source_span) = desc.source_span {
        reflected = reflected.source_span(source_span);
    }
    reflected
}

fn native_function_module(name: &str) -> Option<String> {
    name.rsplit_once("::")
        .map(|(module, _)| module.to_owned())
        .filter(|module| !module.is_empty())
}

fn reflect_effects(effects: &crate::native::EffectSet) -> MethodEffectSet {
    MethodEffectSet {
        reads_host: effects.reads_host(),
        writes_host: effects.writes_host(),
        emits_events: effects.emits_events(),
        reads_time: effects.reads_time(),
        uses_random: effects.uses_random(),
        reads_io: effects.reads_io(),
        writes_io: effects.writes_io(),
        reads_reflection: effects.reads_reflection(),
        writes_reflection: effects.writes_reflection(),
        calls_reflection: effects.calls_reflection(),
    }
}

fn reflect_access(access: &crate::native::FunctionAccess) -> MethodAccess {
    MethodAccess::new()
        .public(access.public)
        .reflect_callable(access.reflect_callable)
}

fn reflect_function_effects(effects: &crate::native::EffectSet) -> FunctionEffectSet {
    FunctionEffectSet {
        reads_host: effects.reads_host(),
        writes_host: effects.writes_host(),
        emits_events: effects.emits_events(),
        reads_time: effects.reads_time(),
        uses_random: effects.uses_random(),
        reads_io: effects.reads_io(),
        writes_io: effects.writes_io(),
        reads_reflection: effects.reads_reflection(),
        writes_reflection: effects.writes_reflection(),
        calls_reflection: effects.calls_reflection(),
    }
}

fn reflect_function_access(access: &crate::native::FunctionAccess) -> ReflectFunctionAccess {
    ReflectFunctionAccess::new()
        .public(access.public)
        .reflect_visible(access.reflect_visible)
        .reflect_callable(access.reflect_callable)
}

fn type_hint_display(hint: &TypeHint) -> String {
    match hint {
        TypeHint::Any => "any".to_owned(),
        TypeHint::Null => "null".to_owned(),
        TypeHint::Bool => "bool".to_owned(),
        TypeHint::Int => "int".to_owned(),
        TypeHint::Float => "float".to_owned(),
        TypeHint::String => "string".to_owned(),
        TypeHint::Array => "array".to_owned(),
        TypeHint::Map => "map".to_owned(),
        TypeHint::Set => "set".to_owned(),
        TypeHint::PathProxy => "path_proxy".to_owned(),
        TypeHint::Record(key) | TypeHint::Enum(key) | TypeHint::Host(key) => key.name.clone(),
        TypeHint::Trait(name) => name.clone(),
        TypeHint::Function => "function".to_owned(),
    }
}

fn find_type_mut<'a>(types: &'a mut [TypeDesc], key: &TypeKey) -> Option<&'a mut TypeDesc> {
    types.iter_mut().find(|desc| desc.key == *key)
}
