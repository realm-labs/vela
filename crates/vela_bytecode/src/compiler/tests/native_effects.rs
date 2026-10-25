use super::*;
use vela_common::{Capability, CapabilitySet};
use vela_registry::{DefinitionRegistry, EffectSet, FunctionDef, FunctionSignature, ParamDef};

#[test]
fn adding_value_native_calls_expands_observed_and_binding_capabilities() {
    let effects = EffectSet {
        host_read: true,
        host_write: true,
        ..EffectSet::default()
    };
    let before = compile_native_package(effects, "pub fn main() -> i64 { return 1; }");
    let after = compile_native_package(
        effects,
        "pub fn main() -> i64 { return test::native_value(1); }",
    );
    let before_binding = before
        .binding_schema()
        .callables()
        .next()
        .expect("old binding");
    let after_binding = after
        .binding_schema()
        .callables()
        .next()
        .expect("new binding");
    let expected = CapabilitySet::new().with(Capability::HostWrite);
    assert_eq!(before_binding.identity, after_binding.identity);
    assert_eq!(
        after_binding
            .required_capabilities
            .difference(before_binding.required_capabilities),
        expected
    );
    assert_ne!(
        before_binding.contract_fingerprint,
        after_binding.contract_fingerprint
    );
    assert_ne!(
        before.binding_schema().checksum(),
        after.binding_schema().checksum()
    );
    let before_package = &before.package_metadata().expect("old package").packages()[0];
    let after_package = &after.package_metadata().expect("new package").packages()[0];
    assert!(before_package.observed_capabilities().is_empty());
    assert_eq!(
        after_package
            .observed_capabilities()
            .difference(before_package.observed_capabilities()),
        expected
    );
}

#[test]
fn native_host_effects_in_callbacks_remain_in_package_and_binding_summaries() {
    let program = compile_native_package(
        EffectSet {
            host_write: true,
            ..EffectSet::default()
        },
        "pub fn callback() { return || test::native_value(1); }",
    );
    let expected = CapabilitySet::new().with(Capability::HostWrite);
    assert_eq!(
        program.package_metadata().expect("package").packages()[0].observed_capabilities(),
        expected
    );
    let callback = program
        .binding_schema()
        .callables()
        .next()
        .expect("callback binding");
    assert!(callback.effects.host_write);
    assert_eq!(callback.required_capabilities, expected);
}

#[cfg(feature = "artifact-codec")]
#[test]
fn portable_native_host_effect_capability_summaries_round_trip() {
    let registry = native_registry(EffectSet {
        host_read: true,
        host_write: true,
        ..EffectSet::default()
    });
    let program = compile_test_program_with_registry(
        SourceId::new(611),
        "fn helper() -> i64 { return test::native_value(1); } pub fn main() -> i64 { return helper(); }",
        registry.compile_view(),
    ).expect("portable native fixture");
    let schema = program.binding_schema().clone();
    let artifact =
        crate::PortableProgramArtifact::from_compiled(program).expect("portable artifact");
    let decoded =
        crate::PortableProgramArtifact::decode(&artifact.encode().expect("encode artifact"))
            .expect("decode artifact")
            .into_compiled();
    assert_eq!(decoded.binding_schema, schema);
    let expected = CapabilitySet::new().with(Capability::HostWrite);
    for function in decoded.bytecode.functions() {
        assert_eq!(function.verified_capabilities(), Some(expected));
    }
    assert!(
        decoded
            .binding_schema
            .callables()
            .next()
            .expect("decoded binding")
            .effects
            .host_write
    );
}

fn native_registry(effects: EffectSet) -> DefinitionRegistry {
    let mut registry = DefinitionRegistry::new();
    registry
        .register_function(
            FunctionDef::new(
                DefPath::function("host", ["test"], "native_value"),
                FunctionSignature::new([ParamDef::new("value", Some("i64"))], Some("i64")),
            )
            .effects(effects),
        )
        .expect("value-only native definition");
    registry
}

fn compile_native_package(effects: EffectSet, source: &str) -> CompiledProgram {
    let package = PackageId::anonymous();
    let sources = vela_hir::source_ingestion::build_module_source_set(&[ModuleSource::new(
        SourceId::new(610),
        package.clone(),
        ModulePath::root(),
        source,
    )])
    .expect("package source");
    let registry = native_registry(effects);
    compile_package_program(PackageProgramCompilationRequest {
        sources: &sources,
        options: &CompilerOptions::default(),
        registry: Some(registry.compile_view()),
        roots: &std::collections::BTreeSet::from([package.clone()]),
        packages: &[crate::PackageCompilationInput {
            id: package,
            version: vela_package::PackageVersion::new("1.0.0").expect("package version"),
            declared_capabilities: CapabilitySet::new(),
            manifest_path: "vela.toml".into(),
        }],
    })
    .expect("compile native package metadata")
}

#[test]
fn native_host_effects_reach_package_and_binding_capability_ceilings() {
    for (effects, expected) in [
        (EffectSet::default(), CapabilitySet::new()),
        (
            EffectSet {
                host_read: true,
                ..EffectSet::default()
            },
            CapabilitySet::new().with(Capability::HostRead),
        ),
        (
            EffectSet {
                host_write: true,
                ..EffectSet::default()
            },
            CapabilitySet::new().with(Capability::HostWrite),
        ),
        (
            EffectSet {
                host_read: true,
                host_write: true,
                ..EffectSet::default()
            },
            CapabilitySet::new().with(Capability::HostWrite),
        ),
    ] {
        let program = compile_native_package(
            effects,
            "fn helper() -> i64 { return test::native_value(1); } pub fn main() -> i64 { return helper(); }",
        );
        let package = &program
            .package_metadata()
            .expect("package metadata")
            .packages()[0];
        assert_eq!(package.observed_capabilities(), expected, "{effects:?}");
        assert_eq!(
            package
                .observed_capabilities()
                .difference(package.declared_capabilities()),
            expected
        );
        let main = program
            .binding_schema()
            .callables()
            .next()
            .expect("main binding");
        assert_eq!(main.effects.host_read, effects.host_read);
        assert_eq!(main.effects.host_write, effects.host_write);
        assert_eq!(main.required_capabilities, expected);
        assert_eq!(
            program
                .function_by_id(main.executable)
                .expect("main bytecode")
                .verified_capabilities(),
            Some(expected)
        );
    }
}
