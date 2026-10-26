use std::sync::Arc;

use vela_common::SourceId;

use super::{
    RUST_BINDING_SCHEMA_VERSION, RustBindingCallableIdentity, RustBindingParameterDefault,
    RustBindingType, RustBindingTypeDefinition, RustBindingVariantFields,
};
use crate::compiler::compile_test_program;

#[test]
fn native_host_effect_bits_and_capability_ceilings_remain_distinct() {
    use super::RustBindingEffectSet;
    use vela_common::{Capability, CapabilitySet};
    use vela_mir::MirEffect;

    let read = RustBindingEffectSet::from(MirEffect {
        host_read: true,
        ..MirEffect::PURE
    });
    let write = RustBindingEffectSet::from(MirEffect {
        host_write: true,
        ..MirEffect::PURE
    });
    let both = RustBindingEffectSet::from(MirEffect {
        host_read: true,
        host_write: true,
        ..MirEffect::PURE
    });
    assert_eq!(read.bits(), 1 << 14);
    assert_eq!(write.bits(), 1 << 15);
    assert_eq!(both.bits(), (1 << 14) | (1 << 15));
    assert_eq!(
        read.required_capabilities(),
        CapabilitySet::new().with(Capability::HostRead)
    );
    assert_eq!(
        write.required_capabilities(),
        CapabilitySet::new().with(Capability::HostWrite)
    );
    assert_eq!(both.required_capabilities(), write.required_capabilities());
    assert!(
        !read
            .required_capabilities()
            .contains_all(write.required_capabilities())
    );
    assert!(
        !RustBindingEffectSet::default()
            .required_capabilities()
            .contains_all(read.required_capabilities())
    );
    assert_eq!(
        RustBindingEffectSet {
            reflection_read: true,
            ..RustBindingEffectSet::default()
        }
        .bits(),
        1 << 6
    );
    assert_eq!(
        RustBindingEffectSet {
            writes_io: true,
            ..RustBindingEffectSet::default()
        }
        .bits(),
        1 << 13
    );
}

#[cfg(feature = "artifact-codec")]
#[test]
fn native_host_effects_round_trip_through_mir_and_schema_codecs() {
    use super::RustBindingEffectSet;
    use vela_mir::MirEffect;

    let baseline = compile_test_program(SourceId::new(508), "pub fn value() -> i64 { return 1; }")
        .expect("schema fixture");
    for (host_read, host_write) in [(true, false), (false, true), (true, true)] {
        let mir = MirEffect {
            host_read,
            host_write,
            ..MirEffect::external_call()
        };
        let encoded = bincode::serialize(&mir).expect("encode MIR effect");
        assert_eq!(
            bincode::deserialize::<MirEffect>(&encoded).expect("decode MIR effect"),
            mir
        );
        let effects = RustBindingEffectSet::from(mir);
        let encoded = bincode::serialize(&effects).expect("encode binding effect");
        assert_eq!(
            bincode::deserialize::<RustBindingEffectSet>(&encoded).expect("decode binding effect"),
            effects
        );

        let mut schema = baseline.binding_schema().as_ref().clone();
        let callable = schema.callables_mut().next().expect("public binding");
        callable.effects = effects;
        callable.required_capabilities = effects.required_capabilities();
        callable.refresh_contract_fingerprint();
        schema.refresh_checksum();
        let encoded = bincode::serialize(&schema).expect("encode binding schema");
        let decoded = bincode::deserialize::<super::RustBindingSchema>(&encoded)
            .expect("decode binding schema");
        assert_eq!(decoded, schema);
        assert_ne!(decoded.checksum(), baseline.binding_schema().checksum());
        assert_eq!(
            decoded
                .callables()
                .next()
                .expect("decoded binding")
                .required_capabilities,
            effects.required_capabilities()
        );
    }
}

#[cfg(feature = "artifact-codec")]
#[test]
fn schema_rejects_obsolete_versions_before_decoding_binding_facts() {
    for version in 1_u32..=6 {
        let error = bincode::deserialize::<super::RustBindingSchema>(&version.to_le_bytes())
            .expect_err("obsolete schema must reject before reading checksum or binding facts");
        assert!(
            error
                .to_string()
                .contains("unsupported Rust binding schema"),
            "{error}"
        );
    }
    assert_eq!(RUST_BINDING_SCHEMA_VERSION, 7);
}

#[test]
fn schema_exports_public_functions_and_methods_with_structural_contracts() {
    let program = compile_test_program(
        SourceId::new(501),
        include_str!("../../tests/fixtures/compiler/public_binding_schema.vela"),
    )
    .expect("binding schema source should compile");
    let schema = program.binding_schema();

    assert_eq!(schema.version(), RUST_BINDING_SCHEMA_VERSION);
    let paths = schema
        .callables()
        .map(|callable| callable.public_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(paths, ["Counter::add", "calculate"]);
    let definitions = schema.types().collect::<Vec<_>>();
    assert_eq!(definitions.len(), 1);
    let RustBindingTypeDefinition::Record(counter) = definitions[0] else {
        panic!("Counter should be a record binding")
    };
    assert_eq!(counter.public_path, "Counter");
    assert_eq!(counter.fields.len(), 1);
    assert_eq!(counter.fields[0].name, "value");
    assert_ne!(counter.schema_fingerprint, 0);
    assert!(schema.callables().all(|callable| matches!(
        callable.identity,
        RustBindingCallableIdentity::Function(_) | RustBindingCallableIdentity::Method { .. }
    )));

    let calculate = schema
        .callables()
        .find(|callable| callable.public_path == "calculate")
        .expect("public function binding");
    assert_eq!(calculate.docs.as_deref(), Some("Calculate a value"));
    assert_eq!(calculate.source.source, SourceId::new(501));
    assert_eq!(calculate.parameters.len(), 2);
    assert_eq!(
        calculate.parameters[0].ty,
        RustBindingType::Path {
            segments: Box::new(["Array".to_owned()]),
            arguments: Box::new([RustBindingType::Path {
                segments: Box::new(["i64".to_owned()]),
                arguments: Box::new([]),
            }]),
        }
    );
    assert!(matches!(
        calculate.parameters[1].default,
        RustBindingParameterDefault::VelaExpression { .. }
    ));
    assert_eq!(
        calculate.returns.ty,
        RustBindingType::Path {
            segments: Box::new(["Result".to_owned()]),
            arguments: Box::new([
                RustBindingType::Path {
                    segments: Box::new(["i64".to_owned()]),
                    arguments: Box::new([]),
                },
                RustBindingType::Path {
                    segments: Box::new(["String".to_owned()]),
                    arguments: Box::new([]),
                },
            ]),
        }
    );

    let method = schema
        .callables()
        .find(|callable| callable.public_path == "Counter::add")
        .expect("public method binding");
    assert!(matches!(
        method.identity,
        RustBindingCallableIdentity::Method { .. }
    ));
    assert_eq!(
        method
            .owner
            .as_ref()
            .map(|owner| owner.public_path.as_str()),
        Some("Counter")
    );
    assert_eq!(
        method.parameters[0].ty,
        RustBindingType::Definition {
            type_id: counter.type_id,
            public_path: "Counter".to_owned(),
        }
    );
}

#[test]
fn schema_exports_enum_shapes_and_fingerprints_type_changes() {
    let first = compile_test_program(
        SourceId::new(506),
        r#"
pub enum Choice { None, One(value: i64), Named { value: i64 } }
pub fn echo(value: Choice) -> Choice { return value; }
"#,
    )
    .expect("first enum schema");
    let changed = compile_test_program(
        SourceId::new(507),
        r#"
pub enum Choice { None, One(value: String), Named { value: i64 } }
pub fn echo(value: Choice) -> Choice { return value; }
"#,
    )
    .expect("changed enum schema");

    let RustBindingTypeDefinition::Enum(item) =
        first.binding_schema().types().next().expect("enum binding")
    else {
        panic!("Choice should be an enum binding")
    };
    assert_eq!(item.variants.len(), 3);
    assert!(matches!(
        item.variants[0].fields,
        RustBindingVariantFields::Unit
    ));
    assert!(matches!(
        item.variants[1].fields,
        RustBindingVariantFields::Tuple(_)
    ));
    assert!(matches!(
        item.variants[2].fields,
        RustBindingVariantFields::Record(_)
    ));
    assert_ne!(
        first.binding_schema().checksum(),
        changed.binding_schema().checksum()
    );
}

#[test]
fn schema_fingerprint_excludes_source_movement_but_tracks_contract_changes() {
    let first = compile_test_program(
        SourceId::new(502),
        "pub fn score(value: i64 = 1) -> i64 { return value + 1; }",
    )
    .expect("first schema");
    let moved = compile_test_program(
        SourceId::new(503),
        "\n\n\npub fn score(value: i64 = 1) -> i64 { return value + 2; }",
    )
    .expect("moved schema");
    let changed = compile_test_program(
        SourceId::new(504),
        "pub fn score(value: String = \"1\") -> i64 { return 1; }",
    )
    .expect("changed schema");

    assert_eq!(
        first.binding_schema().checksum(),
        moved.binding_schema().checksum()
    );
    assert_ne!(
        first.binding_schema().checksum(),
        changed.binding_schema().checksum()
    );
    assert_ne!(
        first
            .binding_schema()
            .callables()
            .next()
            .expect("first")
            .source,
        moved
            .binding_schema()
            .callables()
            .next()
            .expect("moved")
            .source
    );
}

#[test]
fn schema_uses_transitive_effects_and_is_carried_into_linked_artifact() {
    let program = compile_test_program(
        SourceId::new(505),
        r#"
state counter: i64 = 1;
fn read_counter() { return counter; }
pub fn current() { return read_counter(); }
"#,
    )
    .expect("effect schema source should compile");
    let schema = Arc::clone(program.binding_schema());
    let current = schema.callables().next().expect("current binding");
    assert!(current.effects.script_call);
    assert!(current.effects.state_read);

    let artifact = crate::Linker::new()
        .link_compiled_program(program)
        .expect("binding schema program should link");
    assert!(Arc::ptr_eq(&schema, artifact.binding_schema()));
}
