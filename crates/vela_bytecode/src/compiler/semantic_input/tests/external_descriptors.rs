use vela_def::{DefPath, FieldId, FunctionId, TypeId, VariantId};
use vela_mir::{
    CompileConstructorTarget, CompileConstructorValue, CompilePositionalPolicy, CompileTypeClass,
};
use vela_registry::{
    DefinitionRegistry, FieldDef, FunctionDef, FunctionSignature, ParamDef, TypeDef, TypeKindDef,
    VariantDef,
};

use super::{FixtureRoots, prepare_source, prepare_source_with_registry};
use crate::compiler::error::CompileErrorKind;

#[test]
fn provided_registry_does_not_enable_runtime_only_reflection_natives() {
    let registry = DefinitionRegistry::new();
    let error = prepare_source_with_registry(
        "fn main() { return reflect::functions(); }",
        FixtureRoots::Program,
        registry.compile_view(),
    )
    .expect_err("reflection must remain disabled unless the provided registry enables it");

    assert!(matches!(
        error.kind,
        CompileErrorKind::SemanticDiagnostics(ref diagnostics)
            if diagnostics.iter().any(|diagnostic| {
                diagnostic.code.as_deref() == Some("compiler::unresolved_native_function")
                    && diagnostic.message.contains("reflect::functions")
            })
    ));
}

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
    assert_eq!(owner.canonical_name, "host::Player");
    assert_eq!(owner.runtime_name, "Player");
    assert!(fixture.input.targets().type_by_name("Player").is_none());
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

    assert_eq!(descriptor.canonical_name, "host::game::Outcome");
    assert_eq!(descriptor.runtime_name, "game::Outcome");
    assert!(targets.type_by_name("game::Outcome").is_none());
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

#[test]
fn duplicate_default_field_orders_share_canonical_analysis_and_target_slots() {
    let mut registry = DefinitionRegistry::new();
    let ty = TypeId::new(8_201);
    let zeta = FieldId::new(8_202);
    let alpha = FieldId::new(8_203);
    registry
        .register_type(
            TypeDef::new(DefPath::ty("host", ["game"], "Payload"))
                .with_id(ty)
                .kind(TypeKindDef::ScriptStruct),
        )
        .expect("payload type");
    registry
        .register_field(
            FieldDef::new(DefPath::field("host", ["game"], "Payload", "zeta"), ty)
                .with_id(zeta)
                .type_hint(Some("i64")),
        )
        .expect("zeta field");
    registry
        .register_field(
            FieldDef::new(DefPath::field("host", ["game"], "Payload", "alpha"), ty)
                .with_id(alpha)
                .type_hint(Some("i64")),
        )
        .expect("alpha field");

    let fixture = prepare_source_with_registry(
        "fn main() { return game::Payload { zeta: 2, alpha: 1 }; }",
        FixtureRoots::Program,
        registry.compile_view(),
    )
    .expect("duplicate default orders should canonicalize");
    let targets = fixture.input.targets();
    let descriptor = targets.type_descriptor(ty).expect("payload descriptor");
    assert_eq!(descriptor.fields, [alpha, zeta]);
    assert_eq!(
        targets
            .field_descriptor(alpha)
            .expect("alpha descriptor")
            .declaration_order,
        0
    );
    assert_eq!(
        targets
            .field_descriptor(zeta)
            .expect("zeta descriptor")
            .declaration_order,
        1
    );

    let expression = fixture.constructor_expressions[0].1;
    let constructor = targets
        .compilation_roots()
        .find_map(|(function, _)| targets.function_targets(function)?.constructor(expression))
        .expect("payload constructor placement");
    let CompileConstructorTarget::Record { fields, .. } = constructor else {
        panic!("expected registered record constructor");
    };
    assert_eq!(fields.len(), 2);
    assert_eq!((fields[0].field, fields[0].parameter), (alpha, 0));
    assert_eq!((fields[1].field, fields[1].parameter), (zeta, 1));
    assert!(matches!(
        fields[0].value,
        CompileConstructorValue::Explicit {
            source_index: 1,
            ..
        }
    ));
    assert!(matches!(
        fields[1].value,
        CompileConstructorValue::Explicit {
            source_index: 0,
            ..
        }
    ));
}

#[test]
fn duplicate_default_variant_orders_share_canonical_analysis_and_target_slots() {
    let mut registry = DefinitionRegistry::new();
    let ty = TypeId::new(8_301);
    let zed = VariantId::new(8_302);
    let alpha = VariantId::new(8_303);
    let zeta = FieldId::new(8_304);
    let alpha_field = FieldId::new(8_305);
    registry
        .register_type(
            TypeDef::new(DefPath::ty("host", ["game"], "Outcome"))
                .with_id(ty)
                .kind(TypeKindDef::ScriptEnum),
        )
        .expect("outcome type");
    registry
        .register_variant(
            VariantDef::new(DefPath::variant("host", ["game"], "Outcome", "Zed"), ty).with_id(zed),
        )
        .expect("Zed variant");
    registry
        .register_variant(
            VariantDef::new(DefPath::variant("host", ["game"], "Outcome", "Alpha"), ty)
                .with_id(alpha),
        )
        .expect("Alpha variant");
    registry
        .register_field(
            FieldDef::new(DefPath::field("host", ["game"], "Outcome::Zed", "zeta"), ty)
                .with_id(zeta)
                .variant_owner(zed)
                .type_hint(Some("i64")),
        )
        .expect("Zed zeta field");
    registry
        .register_field(
            FieldDef::new(
                DefPath::field("host", ["game"], "Outcome::Zed", "alpha"),
                ty,
            )
            .with_id(alpha_field)
            .variant_owner(zed)
            .type_hint(Some("i64")),
        )
        .expect("Zed alpha field");

    let fixture = prepare_source_with_registry(
        "fn main() { return game::Outcome::Zed { zeta: 2, alpha: 1 }; }",
        FixtureRoots::Program,
        registry.compile_view(),
    )
    .expect("duplicate enum orders should canonicalize");
    let targets = fixture.input.targets();
    assert_eq!(
        targets
            .type_descriptor(ty)
            .expect("outcome descriptor")
            .variants,
        [alpha, zed]
    );
    assert_eq!(
        targets
            .variant_descriptor(alpha)
            .expect("Alpha descriptor")
            .declaration_order,
        0
    );
    let descriptor = targets.variant_descriptor(zed).expect("Zed descriptor");
    assert_eq!(descriptor.declaration_order, 1);
    assert_eq!(descriptor.fields, [alpha_field, zeta]);

    let expression = fixture.constructor_expressions[0].1;
    let constructor = targets
        .compilation_roots()
        .find_map(|(function, _)| targets.function_targets(function)?.constructor(expression))
        .expect("Zed constructor placement");
    let CompileConstructorTarget::Variant {
        variant, fields, ..
    } = constructor
    else {
        panic!("expected registered variant constructor");
    };
    assert_eq!(*variant, zed);
    assert_eq!((fields[0].field, fields[0].parameter), (alpha_field, 0));
    assert_eq!((fields[1].field, fields[1].parameter), (zeta, 1));
    assert!(matches!(
        fields[0].value,
        CompileConstructorValue::Explicit {
            source_index: 1,
            ..
        }
    ));
    assert!(matches!(
        fields[1].value,
        CompileConstructorValue::Explicit {
            source_index: 0,
            ..
        }
    ));
}
