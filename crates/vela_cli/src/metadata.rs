use vela_engine::engine::Engine;
use vela_engine::interop::BoundaryMode;
use vela_engine::native::{EffectSet, TypeHint};
use vela_engine::service::{ServiceMethodDescriptor, ServiceSetSchema};
use vela_language_service::{
    SchemaArtifact, SchemaServiceFact, SchemaServiceMethodFact, SchemaServiceParameterFact,
    SchemaServiceSetFact,
};

pub(crate) fn schema_json(engine: &Engine) -> Result<String, Box<dyn std::error::Error>> {
    let facts = engine.tooling_registry_facts()?;
    let artifact = match engine.service_set_schema() {
        Some(schema) => {
            SchemaArtifact::from_registry_facts(&facts).with_service_set(service_set_fact(schema))
        }
        None => SchemaArtifact::from_registry_facts(&facts),
    };
    artifact
        .to_json()
        .map_err(|error| error.message().to_owned().into())
}

fn service_set_fact(schema: &ServiceSetSchema) -> SchemaServiceSetFact {
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
                service.methods().iter().map(service_method_fact),
            )
        }),
    )
}

fn service_method_fact(method: &ServiceMethodDescriptor) -> SchemaServiceMethodFact {
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
            }),
        type_hint(&method.callable.returns.ty),
    )
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
    use vela_engine::engine::Engine;
    use vela_macros::{service, service_set};

    use super::schema_json;

    #[service(path = "cli_test::handler")]
    pub trait HandlerService: Send + Sync {
        fn handle(&self, value: i64) -> i64;
    }

    pub struct RustHandlerService;

    impl HandlerService for RustHandlerService {
        fn handle(&self, value: i64) -> i64 {
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
        let engine = CliServices::register_types(Engine::builder())
            .build()
            .expect("service engine");
        let json = schema_json(&engine).expect("schema JSON");

        assert!(json.contains(r#""serviceSet""#));
        assert!(json.contains(r#""path": "cli_test::handler""#));
        assert!(json.contains(r#""name": "handle""#));
        assert!(json.contains(r#""typeBindings""#));
    }
}
