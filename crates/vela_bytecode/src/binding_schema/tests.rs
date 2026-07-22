use std::sync::Arc;

use vela_common::{CallableAsyncness, SourceId};

use super::{
    RUST_BINDING_SCHEMA_VERSION, RustBindingCallableIdentity, RustBindingParameterDefault,
    RustBindingType, RustBindingTypeDefinition, RustBindingVariantFields,
};
use crate::compiler::compile_test_program;

#[test]
fn schema_exports_public_functions_and_methods_with_structural_contracts() {
    let program = compile_test_program(
        SourceId::new(501),
        r#"
#[doc("Calculate a value")]
pub async fn calculate(input: Array<i64>, scale: i64 = 2) -> Result<i64, String> {
return scale;
}

fn hidden() { return 0; }

pub struct Counter { value: i64 }

impl Counter {
pub fn add(self, amount: i64) -> i64 { return amount; }
fn secret(self) -> i64 { return 0; }
}
"#,
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
    assert_eq!(calculate.asyncness, CallableAsyncness::Async);
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
