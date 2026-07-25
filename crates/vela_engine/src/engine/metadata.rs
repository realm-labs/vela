#[cfg(feature = "schema-artifact")]
use std::collections::BTreeSet;

#[cfg(feature = "schema-artifact")]
use crate::interop::{BoundaryMode, ReturnMode};
#[cfg(feature = "schema-artifact")]
use crate::native::{EffectSet, TypeHint};
#[cfg(feature = "schema-artifact")]
use crate::service::{ServiceMethodDescriptor, ServiceSetSchema};

use super::Engine;

impl Engine {
    #[must_use]
    pub fn service_set_schema(&self) -> Option<&crate::service::ServiceSetSchema> {
        self.service_set_schema.as_deref()
    }

    /// Copies the sealed compiler registry into the metadata model shared by
    /// CLI schema export and native language tooling.
    pub fn tooling_registry_facts(
        &self,
    ) -> Result<vela_analysis::registry::RegistryFacts, vela_registry::RegistryDeclarationSlotError>
    {
        vela_analysis::registry::RegistryFacts::from_compile_view(self.compiler_registry())
    }

    /// Projects this engine's sealed compiler registry and service set into the
    /// stable metadata artifact consumed by offline compilers and language
    /// tooling.
    ///
    /// The artifact describes the ABI that scripts may compile against. It
    /// does not contain executable bytecode and must still be paired with a
    /// separately validated program artifact before deployment.
    #[cfg(feature = "schema-artifact")]
    pub fn tooling_schema_artifact(
        &self,
    ) -> Result<vela_language_service::SchemaArtifact, vela_registry::RegistryDeclarationSlotError>
    {
        let facts = self.tooling_registry_facts()?;
        let artifact = vela_language_service::SchemaArtifact::from_registry_facts(&facts);
        Ok(match self.service_set_schema() {
            Some(schema) => artifact.with_service_set(service_set_fact(schema, self)),
            None => artifact,
        })
    }
}

#[cfg(feature = "schema-artifact")]
fn service_set_fact(
    schema: &ServiceSetSchema,
    engine: &Engine,
) -> vela_language_service::SchemaServiceSetFact {
    let produced_borrows = produced_borrow_types(schema);
    let bindings = engine.type_bindings();
    vela_language_service::SchemaServiceSetFact::new(
        hex_u128(schema.id().get()),
        schema.path(),
        hex_u64(schema.abi_fingerprint().get()),
        hex_u64(schema.type_binding_checksum().get()),
        schema.named_services().map(|(member, service)| {
            vela_language_service::SchemaServiceFact::new(
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

#[cfg(feature = "schema-artifact")]
fn service_method_fact(
    method: &ServiceMethodDescriptor,
    bindings: &crate::type_binding::TypeBindingRegistry,
    produced_borrows: &BTreeSet<vela_common::InteropTypeId>,
) -> vela_language_service::SchemaServiceMethodFact {
    vela_language_service::SchemaServiceMethodFact::new(
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
                vela_language_service::SchemaServiceParameterFact::new(
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

#[cfg(feature = "schema-artifact")]
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

#[cfg(feature = "schema-artifact")]
fn host_origins(
    mode: BoundaryMode,
    hint: &TypeHint,
    bindings: &crate::type_binding::TypeBindingRegistry,
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

#[cfg(feature = "schema-artifact")]
fn type_hint_binding_id(hint: &TypeHint) -> Option<vela_common::InteropTypeId> {
    let key = match hint {
        TypeHint::Host(key) => key,
        TypeHint::OptionOf(payload) => return type_hint_binding_id(payload),
        TypeHint::ResultOf { ok, .. } => return type_hint_binding_id(ok),
        _ => return None,
    };
    Some(vela_common::InteropTypeId::from_type_id(key.id))
}

#[cfg(feature = "schema-artifact")]
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

#[cfg(feature = "schema-artifact")]
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

#[cfg(feature = "schema-artifact")]
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

#[cfg(feature = "schema-artifact")]
fn generic(name: &str, args: impl IntoIterator<Item = String>) -> String {
    format!(
        "{name}<{}>",
        args.into_iter().collect::<Vec<_>>().join(", ")
    )
}

#[cfg(feature = "schema-artifact")]
fn hex_u128(value: u128) -> String {
    format!("0x{value:032x}")
}

#[cfg(feature = "schema-artifact")]
fn hex_u64(value: u64) -> String {
    format!("0x{value:016x}")
}
