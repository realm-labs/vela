use std::collections::BTreeSet;

use vela_engine::engine::Engine;
use vela_engine::interop::{BoundaryMode, ReturnMode};
use vela_engine::native::{EffectSet, TypeHint};
use vela_engine::service::{ServiceMethodDescriptor, ServiceSetSchema};
use vela_language_service::{
    SchemaArtifact, SchemaServiceFact, SchemaServiceMethodFact, SchemaServiceParameterFact,
    SchemaServiceSetFact,
};

pub(crate) fn schema_json(engine: &Engine) -> Result<String, Box<dyn std::error::Error>> {
    let facts = engine.tooling_registry_facts()?;
    let artifact = match engine.service_set_schema() {
        Some(schema) => SchemaArtifact::from_registry_facts(&facts)
            .with_service_set(service_set_fact(schema, engine)),
        None => SchemaArtifact::from_registry_facts(&facts),
    };
    artifact
        .to_json()
        .map_err(|error| error.message().to_owned().into())
}

fn service_set_fact(schema: &ServiceSetSchema, engine: &Engine) -> SchemaServiceSetFact {
    let produced_borrows = produced_borrow_types(schema);
    let bindings = engine.type_bindings();
    SchemaServiceSetFact::new(
        hex_u128(schema.id().get()),
        schema.path(),
        hex_u64(schema.abi_fingerprint().get()),
        hex_u64(schema.type_binding_checksum().get()),
        schema.named_services().map(|(member, service)| {
            SchemaServiceFact::new(
                hex_u128(service.id().get()),
                member,
                service.path(),
                hex_u64(service.abi_fingerprint().get()),
                service
                    .methods()
                    .iter()
                    .map(|method| service_method_fact(method, &bindings, &produced_borrows)),
            )
        }),
    )
}

fn service_method_fact(
    method: &ServiceMethodDescriptor,
    bindings: &vela_engine::type_binding::TypeBindingRegistry,
    produced_borrows: &BTreeSet<vela_common::InteropTypeId>,
) -> SchemaServiceMethodFact {
    SchemaServiceMethodFact::new(
        hex_u128(method.id.get()),
        method
            .path
            .rsplit("::")
            .next()
            .unwrap_or(method.path.as_str()),
        &method.path,
        method.callable.asyncness == vela_common::CallableAsyncness::Async,
        effect_names(method.callable.effects),
        method
            .callable
            .parameters
            .iter()
            .filter(|parameter| parameter.mode != BoundaryMode::HiddenContext)
            .map(|parameter| {
                SchemaServiceParameterFact::new(
                    &parameter.name,
                    type_hint(&parameter.ty),
                    boundary_mode(parameter.mode),
                )
                .with_host_origins(host_origins(
                    parameter.mode,
                    &parameter.ty,
                    bindings,
                    produced_borrows,
                ))
            }),
        type_hint(&method.callable.returns.ty),
    )
}

fn produced_borrow_types(schema: &ServiceSetSchema) -> BTreeSet<vela_common::InteropTypeId> {
    schema
        .services()
        .iter()
        .flat_map(|service| service.methods())
        .filter_map(|method| {
            matches!(method.callable.returns.mode, ReturnMode::ScopedHost { .. })
                .then(|| type_hint_binding_id(&method.callable.returns.ty))
                .flatten()
        })
        .collect()
}

fn host_origins(
    mode: BoundaryMode,
    hint: &TypeHint,
    bindings: &vela_engine::type_binding::TypeBindingRegistry,
    produced_borrows: &BTreeSet<vela_common::InteropTypeId>,
) -> Vec<&'static str> {
    let host_parameter = matches!(
        mode,
        BoundaryMode::SharedHost
            | BoundaryMode::ExclusiveHost
            | BoundaryMode::StorageDirectedShared
    );
    if !host_parameter {
        return Vec::new();
    }
    let mut origins = vec!["injected"];
    let Some(id) = type_hint_binding_id(hint) else {
        return origins;
    };
    if bindings.get(id).is_some_and(|binding| {
        binding.storage == vela_common::StoragePolicy::Host && !binding.host_constructors.is_empty()
    }) {
        origins.push("constructible");
    }
    if produced_borrows.contains(&id) {
        origins.push("produced_borrow");
    }
    origins
}

fn type_hint_binding_id(hint: &TypeHint) -> Option<vela_common::InteropTypeId> {
    let key = match hint {
        TypeHint::Host(key) => key,
        TypeHint::OptionOf(payload) => return type_hint_binding_id(payload),
        TypeHint::ResultOf { ok, .. } => return type_hint_binding_id(ok),
        _ => return None,
    };
    Some(vela_common::InteropTypeId::from_type_id(key.id))
}

fn effect_names(effects: EffectSet) -> Vec<String> {
    let flags = [
        ("host_read", effects.reads_host() && !effects.writes_host()),
        ("host_write", effects.writes_host()),
        ("event_emit", effects.emits_events()),
        ("time", effects.reads_time()),
        ("random", effects.uses_random()),
        ("io_read", effects.reads_io()),
        ("io_write", effects.writes_io()),
        ("reflection_read", effects.reads_reflection()),
        ("reflection_write", effects.writes_reflection()),
        ("reflection_call", effects.calls_reflection()),
    ];
    let names = flags
        .into_iter()
        .filter(|(_, enabled)| *enabled)
        .map(|(name, _)| name.to_owned())
        .collect::<Vec<_>>();
    if names.is_empty() {
        vec!["pure".to_owned()]
    } else {
        names
    }
}

fn boundary_mode(mode: BoundaryMode) -> &'static str {
    match mode {
        BoundaryMode::Value => "value",
        BoundaryMode::ReadOnlyValueBorrow => "readonly_value_borrow",
        BoundaryMode::StorageDirectedShared => "storage_directed_shared",
        BoundaryMode::SharedHost => "shared_host",
        BoundaryMode::ExclusiveHost => "exclusive_host",
        BoundaryMode::HiddenContext => "hidden_context",
    }
}

fn type_hint(hint: &TypeHint) -> String {
    match hint {
        TypeHint::Any => "Any".to_owned(),
        TypeHint::Primitive(tag) => tag.name().to_owned(),
        TypeHint::Array => "Array".to_owned(),
        TypeHint::ArrayOf(element) => generic("Array", [type_hint(element)]),
        TypeHint::ArrayViewOf(element) => generic("ArrayView", [type_hint(element)]),
        TypeHint::ArrayMutOf { element, mutation } => generic(
            "ArrayMut",
            [type_hint(element), mutation.as_str().to_owned()],
        ),
        TypeHint::Map => "Map".to_owned(),
        TypeHint::MapOf { key, value } => generic("Map", [type_hint(key), type_hint(value)]),
        TypeHint::MapViewOf { key, value } => {
            generic("MapView", [type_hint(key), type_hint(value)])
        }
        TypeHint::MapMutOf {
            key,
            value,
            mutation,
        } => generic(
            "MapMut",
            [
                type_hint(key),
                type_hint(value),
                mutation.as_str().to_owned(),
            ],
        ),
        TypeHint::Set => "Set".to_owned(),
        TypeHint::SetOf(element) => generic("Set", [type_hint(element)]),
        TypeHint::SetViewOf(element) => generic("SetView", [type_hint(element)]),
        TypeHint::SetMutOf { element, mutation } => {
            generic("SetMut", [type_hint(element), mutation.as_str().to_owned()])
        }
        TypeHint::TupleOf(elements) => generic("Tuple", elements.iter().map(type_hint)),
        TypeHint::Iterator => "Iterator".to_owned(),
        TypeHint::IteratorOf(item) => generic("Iterator", [type_hint(item)]),
        TypeHint::OptionOf(payload) => generic("Option", [type_hint(payload)]),
        TypeHint::ResultOf { ok, err } => generic("Result", [type_hint(ok), type_hint(err)]),
        TypeHint::PathProxy => "PathProxy".to_owned(),
        TypeHint::Record(key) | TypeHint::Enum(key) | TypeHint::Host(key) => key.name.clone(),
        TypeHint::Trait(path) => path.clone(),
        TypeHint::Function => "Function".to_owned(),
    }
}

fn generic(name: &str, args: impl IntoIterator<Item = String>) -> String {
    format!(
        "{name}<{}>",
        args.into_iter().collect::<Vec<_>>().join(", ")
    )
}

fn hex_u128(value: u128) -> String {
    format!("0x{value:032x}")
}

fn hex_u64(value: u64) -> String {
    format!("0x{value:016x}")
}

#[cfg(test)]
mod tests {
    use vela_common::HostConstructionLifetime;
    use vela_def::FunctionId;
    use vela_engine::engine::Engine;
    use vela_engine::native::{EffectSet, NativeFunctionDesc, TypeHint};
    use vela_macros::{ScriptHost, service, service_set};
    use vela_vm::error::VmResult;

    use super::schema_json;

    #[derive(ScriptHost)]
    #[script(path = "cli_test::Observed")]
    pub struct Observed {
        #[script(get)]
        value: i64,
    }

    #[vela_macros::script_methods]
    impl Observed {}

    #[service(path = "cli_test::handler")]
    pub trait HandlerService: Send + Sync {
        fn handle(&self, value: i64) -> i64;
        fn inspect(&self, value: &Observed) -> i64;
        fn identity<'a>(&self, value: &'a Observed) -> &'a Observed;
    }

    pub struct RustHandlerService;

    impl HandlerService for RustHandlerService {
        fn handle(&self, value: i64) -> i64 {
            value
        }

        fn inspect(&self, value: &Observed) -> i64 {
            value.value
        }

        fn identity<'a>(&self, value: &'a Observed) -> &'a Observed {
            value
        }
    }

    #[service_set(context = ())]
    pub struct CliServices {
        #[vela::default(RustHandlerService)]
        pub handler: dyn HandlerService,
    }

    #[test]
    fn cli_schema_json_projects_registered_service_metadata() {
        assert_eq!(RustHandlerService.handle(7), 7);
        let observed = Observed { value: 9 };
        assert_eq!(RustHandlerService.inspect(&observed), 9);
        assert!(std::ptr::eq(
            RustHandlerService.identity(&observed),
            &observed,
        ));
        let constructor = NativeFunctionDesc::new("Observed::new", FunctionId::new(0xdead))
            .returns(TypeHint::Host(Observed::vela_host_type_desc().key))
            .effects(EffectSet::pure());
        let engine = CliServices::register_types(Engine::builder().register_rust_type::<Observed>(
            Observed::vela_type_binding().host_constructor_fn(
                HostConstructionLifetime::CallScoped,
                constructor,
                |_args, _host| -> VmResult<Observed> { Ok(Observed { value: 11 }) },
            ),
        ))
        .build()
        .expect("service engine");
        let json = schema_json(&engine).expect("schema JSON");

        assert!(json.contains(r#""serviceSet""#));
        assert!(json.contains(r#""path": "cli_test::handler""#));
        assert!(json.contains(r#""name": "handle""#));
        assert!(json.contains(r#""typeBindings""#));
        assert!(json.contains(r#""mode": "storage_directed_shared""#));
        assert!(json.contains(r#""injected""#));
        assert!(json.contains(r#""constructible""#));
        assert!(json.contains(r#""produced_borrow""#));
    }
}
