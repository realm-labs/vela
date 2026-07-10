use vela_analysis::logical_records::{LogicalRecordFact, LogicalRecordKind};
use vela_mir::{
    CompileFieldTarget, CompileGuardKey, CompileMemberTarget, CompileTypeClass, MirTypeContract,
};

use super::{FixtureRoots, SemanticFixture, prepare_source};

#[test]
fn map_entry_members_materialize_one_standard_descriptor_and_stable_record_slot() {
    let fixture = prepare_source(
        r#"
fn main() {
    let rewards = {"gold": 5, "gem": 6};
    let entries = rewards.entries().collect_array();
    let first = entries[0];
    return first.value;
}
"#,
        FixtureRoots::Program,
    )
    .expect("logical MapEntry semantic input");
    let targets = fixture.input.targets();
    let kind = LogicalRecordKind::MapEntry;
    let manifest = assert_standard_descriptor(&fixture, kind);

    assert_eq!(
        targets
            .type_by_name(&kind.canonical_name())
            .map(|descriptor| descriptor.id),
        Some(kind.type_id())
    );
    assert!(
        targets.type_by_name(kind.runtime_name()).is_none(),
        "logical record descriptors must not be recovered through runtime-name fallback"
    );
    for field in manifest.fields() {
        let descriptor = targets
            .field_descriptor(field.id())
            .expect("MapEntry field descriptor");
        assert_eq!(descriptor.contract, None);
        assert!(targets.guard(CompileGuardKey::Field(field.id())).is_none());
    }

    assert_record_slot(&fixture, "value", kind);
}

#[test]
fn reflection_member_use_closes_nested_standard_descriptors_and_contracts() {
    let fixture = prepare_source(
        r#"
fn main() {
    let function = reflect::functions()[0];
    let access = function.access;
    let effects = function.effects;
    let param = function.params[0];
    return access.reflect_visible && effects.uses_random && param.defaulted;
}
"#,
        FixtureRoots::Program,
    )
    .expect("nested reflection logical-record semantic input");

    for kind in [
        LogicalRecordKind::ReflectFunction,
        LogicalRecordKind::ReflectFunctionAccess,
        LogicalRecordKind::ReflectEffectSet,
        LogicalRecordKind::ReflectParam,
    ] {
        assert_standard_descriptor(&fixture, kind);
    }

    assert_record_slot(&fixture, "access", LogicalRecordKind::ReflectFunction);
    assert_record_slot(&fixture, "effects", LogicalRecordKind::ReflectFunction);
    assert_record_slot(&fixture, "params", LogicalRecordKind::ReflectFunction);
    assert_record_slot(
        &fixture,
        "reflect_visible",
        LogicalRecordKind::ReflectFunctionAccess,
    );
    assert_record_slot(&fixture, "uses_random", LogicalRecordKind::ReflectEffectSet);
    assert_record_slot(&fixture, "defaulted", LogicalRecordKind::ReflectParam);

    let targets = fixture.input.targets();
    let function = LogicalRecordFact::fixed(LogicalRecordKind::ReflectFunction);
    let access = LogicalRecordFact::fixed(LogicalRecordKind::ReflectFunctionAccess);
    let param = LogicalRecordFact::fixed(LogicalRecordKind::ReflectParam);
    assert_eq!(
        targets
            .field_descriptor(
                function
                    .field("access")
                    .expect("ReflectFunction.access")
                    .id(),
            )
            .and_then(|field| field.contract.as_ref()),
        Some(&MirTypeContract::Shape {
            type_id: access.type_id(),
            shape: access.shape(),
        })
    );
    assert_eq!(
        targets
            .field_descriptor(
                function
                    .field("params")
                    .expect("ReflectFunction.params")
                    .id(),
            )
            .and_then(|field| field.contract.as_ref()),
        Some(&MirTypeContract::Array(Some(Box::new(
            MirTypeContract::Shape {
                type_id: param.type_id(),
                shape: param.shape(),
            },
        ))))
    );
}

fn assert_standard_descriptor(
    fixture: &SemanticFixture,
    kind: LogicalRecordKind,
) -> LogicalRecordFact {
    let manifest = LogicalRecordFact::manifest(kind);
    let targets = fixture.input.targets();
    let descriptor = targets
        .type_descriptor(kind.type_id())
        .unwrap_or_else(|| panic!("{} type descriptor", kind.runtime_name()));
    assert_eq!(descriptor.class, CompileTypeClass::Standard);
    assert_eq!(descriptor.canonical_name, kind.canonical_name());
    assert_eq!(descriptor.shape, Some(manifest.shape()));
    assert_eq!(
        descriptor.fields,
        manifest
            .fields()
            .map(|field| field.id())
            .collect::<Vec<_>>()
    );
    for field in manifest.fields() {
        let descriptor = targets
            .field_descriptor(field.id())
            .unwrap_or_else(|| panic!("{}.{} descriptor", kind.runtime_name(), field.name()));
        assert_eq!(descriptor.owner, manifest.type_id());
        assert_eq!(descriptor.variant, None);
        assert_eq!(descriptor.name, field.name());
        assert_eq!(descriptor.declaration_order, field.canonical_slot());
        assert_eq!(descriptor.host_runtime, None);
    }
    manifest
}

fn assert_record_slot(fixture: &SemanticFixture, member: &str, kind: LogicalRecordKind) {
    let manifest = LogicalRecordFact::manifest(kind);
    let field = manifest
        .field(member)
        .unwrap_or_else(|| panic!("{}.{} manifest field", kind.runtime_name(), member));
    let expression = fixture
        .member_expressions
        .iter()
        .find_map(|(_, expression, name)| (name == member).then_some(*expression))
        .unwrap_or_else(|| panic!("{member} member expression"));
    let targets = fixture.input.targets();
    let function = targets
        .compilation_roots()
        .next()
        .expect("main compilation root")
        .0;
    assert_eq!(
        targets
            .function_targets(function)
            .expect("main function targets")
            .member(expression),
        Some(&CompileMemberTarget::ScriptField(
            CompileFieldTarget::RecordSlot {
                type_id: manifest.type_id(),
                shape: manifest.shape(),
                field: field.id(),
            },
        ))
    );
}
