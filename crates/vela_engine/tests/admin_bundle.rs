#![cfg(feature = "artifact-codec")]

use vela_bytecode::PortableProgramArtifact;
use vela_common::{CallableAsyncness, Capability, CapabilitySet};
use vela_engine::admin::{
    AdminScriptAbi, PortableAdminBundleError, PortableAdminDiagnosticSource,
    PortableAdminScriptBundle,
};
use vela_engine::engine::Engine;
use vela_engine::runtime::{CallArgs, CallOptions, Runtime};
use vela_package::PackageId;
use vela_vm::owned_value::OwnedValue;

fn operation_abi(parameter_count: u32) -> AdminScriptAbi {
    AdminScriptAbi::new(
        0xa11c_e0de_0000_0001,
        vela_def::script_function_id(PackageId::anonymous().as_str(), "execute"),
        "execute",
        CallableAsyncness::Sync,
        parameter_count,
        CapabilitySet::new(),
    )
}

#[test]
fn portable_admin_bundle_round_trips_validates_and_executes_without_source_compilation() {
    let source = "fn execute(left: i64, right: i64) -> i64 { return left + right; }";
    let build_engine = Engine::builder().build().expect("build compiler engine");
    let artifact = PortableProgramArtifact::from_compiled(
        build_engine
            .compile_source(source)
            .expect("offline operation compile"),
    )
    .expect("portable operation artifact");
    let abi = operation_abi(2);
    let bundle = PortableAdminScriptBundle::build(
        0x1234_5678_9abc_def0,
        &abi,
        artifact,
        [PortableAdminDiagnosticSource::new("operation.vela", source)],
    )
    .expect("portable admin bundle");

    let first = bundle.encode().expect("first encoding");
    let second = bundle.encode().expect("second encoding");
    assert_eq!(first, second);
    let checksum = bundle.checksum();
    let mut corrupt = first.clone();
    *corrupt.last_mut().expect("payload byte") ^= 0x20;
    assert!(matches!(
        PortableAdminScriptBundle::decode(&corrupt),
        Err(PortableAdminBundleError::ChecksumMismatch)
    ));
    let mut foreign = first.clone();
    foreign[8..12].copy_from_slice(&2_u32.to_le_bytes());
    assert!(matches!(
        PortableAdminScriptBundle::decode(&foreign),
        Err(PortableAdminBundleError::UnsupportedFormat {
            expected: 1,
            actual: 2,
        })
    ));

    let decoded = PortableAdminScriptBundle::decode(&first).expect("decode admin bundle");
    assert_eq!(decoded.checksum(), checksum);
    assert_eq!(decoded.diagnostics()[0].path(), "operation.vela");
    let receiving_engine = Engine::builder().build().expect("receiving Engine");
    assert!(matches!(
        decoded
            .clone()
            .load(&receiving_engine, 0x1234_5678_9abc_def1, &abi),
        Err(PortableAdminBundleError::HostSchemaHashMismatch { .. })
    ));
    assert!(matches!(
        decoded
            .clone()
            .load(&receiving_engine, 0x1234_5678_9abc_def0, &operation_abi(1),),
        Err(PortableAdminBundleError::AbiMismatch { .. })
    ));

    let loaded = decoded
        .load(&receiving_engine, 0x1234_5678_9abc_def0, &abi)
        .expect("load admin bundle");
    assert!(loaded.observed_capabilities().is_empty());
    let mut runtime = Runtime::from_linked_artifact(receiving_engine, loaded.artifact().clone())
        .expect("portable operation Runtime");
    let value = runtime
        .call(
            loaded.abi().symbol(),
            CallArgs::from_positional([OwnedValue::i64(20), OwnedValue::i64(22)]),
            CallOptions::unbounded(),
        )
        .expect("execute portable operation");
    assert_eq!(
        runtime.value_to_owned(&value).expect("detach result"),
        OwnedValue::i64(42)
    );
}

#[test]
fn portable_admin_bundle_rejects_entry_contract_mismatch_at_build_time() {
    let engine = Engine::builder().build().expect("compiler Engine");
    let artifact = PortableProgramArtifact::from_compiled(
        engine
            .compile_source("fn execute(value: i64) -> i64 { return value; }")
            .expect("compile operation"),
    )
    .expect("portable artifact");
    assert!(matches!(
        PortableAdminScriptBundle::build(7, &operation_abi(2), artifact, []),
        Err(PortableAdminBundleError::EntryParameterCountMismatch {
            expected: 2,
            actual: 1,
        })
    ));
}

#[test]
fn portable_admin_bundle_enforces_compiler_observed_capabilities_on_load() {
    let source = "fn execute() -> i64 { return math::random(1, 6); }";
    let build_engine = Engine::builder()
        .with_controlled_random(7)
        .capability(Capability::Random)
        .build()
        .expect("build Engine");
    let artifact = PortableProgramArtifact::from_compiled(
        build_engine
            .compile_source(source)
            .expect("compile random operation"),
    )
    .expect("portable random artifact");
    let abi = AdminScriptAbi::new(
        0xa11c_e0de_0000_0002,
        vela_def::script_function_id(PackageId::anonymous().as_str(), "execute"),
        "execute",
        CallableAsyncness::Sync,
        0,
        CapabilitySet::new().with(Capability::Random),
    );
    let restricted = AdminScriptAbi::new(
        0xa11c_e0de_0000_0002,
        vela_def::script_function_id(PackageId::anonymous().as_str(), "execute"),
        "execute",
        CallableAsyncness::Sync,
        0,
        CapabilitySet::new(),
    );
    assert!(matches!(
        PortableAdminScriptBundle::build(9, &restricted, artifact.clone(), []),
        Err(PortableAdminBundleError::CapabilityCeilingExceeded {
            observed,
            ..
        }) if observed.contains(Capability::Random)
    ));
    let bundle =
        PortableAdminScriptBundle::build(9, &abi, artifact, []).expect("capability-sealed bundle");
    let bytes = bundle.encode().expect("encode bundle");

    let inspect_engine = Engine::builder()
        .with_controlled_random(7)
        .build()
        .expect("Engine without Random");
    assert!(matches!(
        PortableAdminScriptBundle::decode(&bytes)
            .expect("decode bundle")
            .load(&inspect_engine, 9, &abi),
        Err(PortableAdminBundleError::MissingCapabilities {
            required,
            ..
        }) if required.contains(Capability::Random)
    ));

    let mutate_engine = Engine::builder()
        .with_controlled_random(7)
        .capability(Capability::Random)
        .build()
        .expect("Engine with Random");
    let loaded = PortableAdminScriptBundle::decode(&bytes)
        .expect("decode bundle")
        .load(&mutate_engine, 9, &abi)
        .expect("capability-compatible load");
    assert!(loaded.observed_capabilities().contains(Capability::Random));
}
