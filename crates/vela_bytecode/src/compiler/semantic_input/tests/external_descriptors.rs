use vela_def::{DefPath, FieldId, FunctionId, TypeId, VariantId};
use vela_mir::{CompilePositionalPolicy, CompileTypeClass};
use vela_registry::{
    DefinitionRegistry, FieldDef, FunctionDef, FunctionSignature, ParamDef, TypeDef, TypeKindDef,
    VariantDef,
};

use super::{FixtureRoots, prepare_source, prepare_source_with_registry};
use crate::compiler::error::CompileErrorKind;

#[test]
fn opaque_script_method_owner_has_stable_external_identity() {
    let fixture = prepare_source(
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
impl BonusSource for Player {
    fn bonus(self, amount) -> i64 { return amount; }
}
fn main(player) { return player.bonus(5); }
"#,
        FixtureRoots::Program,
    )
    .expect("opaque external method owner should build");
    let owner = fixture
        .input
        .targets()
        .type_by_name("host::Player")
        .expect("opaque owner descriptor");

    assert_eq!(owner.class, CompileTypeClass::OpaqueExternal);
    assert!(owner.fields.is_empty());
    assert!(owner.variants.is_empty());
}

#[test]
fn external_enum_descriptor_edges_are_complete() {
    let mut registry = DefinitionRegistry::new();
    let ty = TypeId::new(8_001);
    let variant = VariantId::new(8_002);
    let field = FieldId::new(8_003);
    registry
        .register_type(
            TypeDef::new(DefPath::ty("host", ["game"], "Outcome"))
                .with_id(ty)
                .kind(TypeKindDef::ScriptEnum),
        )
        .expect("external enum type fixture");
    registry
        .register_variant(
            VariantDef::new(DefPath::variant("host", ["game"], "Outcome", "Win"), ty)
                .with_id(variant),
        )
        .expect("external enum variant fixture");
    registry
        .register_field(
            FieldDef::new(
                DefPath::field("host", ["game"], "Outcome::Win", "score"),
                ty,
            )
            .with_id(field)
            .variant_owner(variant)
            .type_hint(Some("i64")),
        )
        .expect("external enum field fixture");

    let fixture = prepare_source_with_registry(
        "fn main(value: game::Outcome) { return value; }",
        FixtureRoots::Program,
        registry.compile_view(),
    )
    .expect("external contract edge should close its descriptor graph");
    let targets = fixture.input.targets();
    let descriptor = targets.type_descriptor(ty).expect("type descriptor");

    assert!(descriptor.fields.is_empty());
    assert_eq!(descriptor.variants, [variant]);
    assert_eq!(
        targets
            .variant_descriptor(variant)
            .expect("external variant descriptor")
            .fields,
        [field]
    );
    assert_eq!(
        targets
            .field_descriptor(field)
            .expect("external field descriptor")
            .variant,
        Some(variant)
    );
}

#[test]
fn external_any_is_erased_but_unknown_hint_is_rejected() {
    let function = FunctionId::new(8_101);
    let mut valid = DefinitionRegistry::new();
    valid
        .register_function(
            FunctionDef::new(
                DefPath::function("host", ["audit"], "send"),
                FunctionSignature::new([ParamDef::new("value", Some("Any"))], None::<String>),
            )
            .with_id(function),
        )
        .expect("valid external function fixture");
    let fixture = prepare_source_with_registry(
        "fn main(value) { return audit::send(value); }",
        FixtureRoots::Program,
        valid.compile_view(),
    )
    .expect("explicit Any should remain an erased external contract");
    let signature = &fixture
        .input
        .targets()
        .function_descriptor(function)
        .expect("external function descriptor")
        .signature;
    assert_eq!(signature.parameters[0].contract, None);
    assert_eq!(
        signature.positional,
        CompilePositionalPolicy::RuntimeChecked
    );

    let mut invalid = DefinitionRegistry::new();
    invalid
        .register_function(FunctionDef::new(
            DefPath::function("host", ["audit"], "send"),
            FunctionSignature::new(
                [ParamDef::new("value", Some("MisspelledType"))],
                None::<String>,
            ),
        ))
        .expect("invalid external function fixture");
    let error = prepare_source_with_registry(
        "fn main(value) { return audit::send(value); }",
        FixtureRoots::Program,
        invalid.compile_view(),
    )
    .expect_err("unknown registry hints must not degrade to Any");
    assert!(matches!(
        error.kind,
        CompileErrorKind::RegistrySnapshot(message)
            if message.contains("MisspelledType")
    ));
}

#[test]
fn oversized_host_runtime_type_id_is_rejected_up_front() {
    let mut registry = DefinitionRegistry::new();
    registry
        .register_type(
            TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "Oversized"))
                .kind(TypeKindDef::Host)
                .host_runtime_id(u128::from(u64::MAX) + 1),
        )
        .expect("oversized external type fixture");

    let error = prepare_source_with_registry(
        "struct SchemaOnly { value: i64 }",
        FixtureRoots::Program,
        registry.compile_view(),
    )
    .expect_err("oversized HostTypeId must fail even when not referenced");
    assert!(matches!(
        error.kind,
        CompileErrorKind::RegistrySnapshot(message)
            if message.contains("outside the u64 HostTypeId range")
    ));
}
