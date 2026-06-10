use vela_def::DefPath;
use vela_reflect::access::FunctionEffectSet;
use vela_reflect::modules::FunctionDesc;
use vela_reflect::registry::TypeRegistry;
use vela_registry::{
    DefinitionRegistry, EffectSet as DefinitionEffectSet, FunctionDef, FunctionSignature, ParamDef,
    RegistryError,
};

pub(crate) fn definition_registry_from_reflect(
    reflect: &TypeRegistry,
    include_reflection_natives: bool,
    include_stdlib: bool,
) -> Result<DefinitionRegistry, RegistryError> {
    let mut registry = DefinitionRegistry::new();
    if include_stdlib {
        vela_stdlib::register_stdlib(&mut registry)?;
    }
    for function in reflect.functions() {
        let def = function_def(function);
        if include_stdlib
            && function.attrs.get("stdlib").is_some()
            && registry.id_for_path(&def.path).is_some()
        {
            continue;
        }
        registry.register_function(def)?;
    }
    if include_reflection_natives {
        register_reflection_native_defs(&mut registry)?;
    }
    Ok(registry)
}

fn function_def(desc: &FunctionDesc) -> FunctionDef {
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
                .map(|param| ParamDef::new(param.name.clone(), param.type_hint.clone())),
            desc.return_type.clone(),
        ),
    )
    .with_id(desc.id)
    .effects(function_effects(&desc.effects))
}

fn source_function_path(package: &str, name: &str) -> DefPath {
    let mut parts = name
        .split("::")
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let function_name = parts.pop().unwrap_or(name);
    DefPath::function(package, parts, function_name)
}

fn function_effects(effects: &FunctionEffectSet) -> DefinitionEffectSet {
    DefinitionEffectSet {
        host_read: effects.reads_host,
        host_write: effects.writes_host,
        reflection_read: effects.reads_reflection,
        reflection_call: effects.calls_reflection,
        event_emit: effects.emits_events,
        time: effects.reads_time,
        random: effects.uses_random,
        io_read: effects.reads_io,
        io_write: effects.writes_io,
    }
}

fn register_reflection_native_defs(registry: &mut DefinitionRegistry) -> Result<(), RegistryError> {
    for (name, params) in REFLECTION_NATIVE_DEFS {
        registry.register_function(FunctionDef::new(
            source_function_path("host", name),
            FunctionSignature::new(
                params
                    .iter()
                    .map(|param| ParamDef::new(*param, None::<String>)),
                None::<String>,
            ),
        ))?;
    }
    Ok(())
}

const REFLECTION_NATIVE_DEFS: &[(&str, &[&str])] = &[
    ("reflect::access", &["target"]),
    ("reflect::attr", &["target", "name"]),
    ("reflect::attrs", &["target"]),
    ("reflect::call", &["target"]),
    ("reflect::docs", &["target"]),
    ("reflect::effects", &["target"]),
    ("reflect::exports", &["target"]),
    ("reflect::field", &["target", "name"]),
    ("reflect::fields", &["target"]),
    ("reflect::function", &["name"]),
    ("reflect::functions", &[]),
    ("reflect::get", &["target", "field"]),
    ("reflect::has_attr", &["target", "name"]),
    ("reflect::has_field", &["target", "name"]),
    ("reflect::has_function", &["name"]),
    ("reflect::has_method", &["target", "name"]),
    ("reflect::has_module", &["name"]),
    ("reflect::has_permission", &["name"]),
    ("reflect::has_trait", &["name"]),
    ("reflect::has_type", &["name"]),
    ("reflect::has_variant", &["target", "name"]),
    ("reflect::id", &["target"]),
    ("reflect::implements", &["target", "trait"]),
    ("reflect::kind", &["target"]),
    ("reflect::method", &["target", "name"]),
    ("reflect::methods", &["target"]),
    ("reflect::module", &["name"]),
    ("reflect::modules", &[]),
    ("reflect::name", &["target"]),
    ("reflect::origin", &["target"]),
    ("reflect::owner", &["target"]),
    ("reflect::params", &["target"]),
    ("reflect::permissions", &[]),
    ("reflect::required_permissions", &["target"]),
    ("reflect::returns", &["target"]),
    ("reflect::set", &["target", "field", "value"]),
    ("reflect::source_span", &["target"]),
    ("reflect::trait_info", &["name"]),
    ("reflect::traits", &["target"]),
    ("reflect::type_info", &["name"]),
    ("reflect::type_of", &["target"]),
    ("reflect::types", &[]),
    ("reflect::variant", &["target"]),
    ("reflect::variant_info", &["target", "name"]),
    ("reflect::variant_is", &["target", "name"]),
    ("reflect::variants", &["target"]),
];
