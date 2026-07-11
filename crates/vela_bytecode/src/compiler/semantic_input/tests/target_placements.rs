use vela_common::{PrimitiveTag, ScalarValue};
use vela_def::DefPath;
use vela_mir::{
    CompileCallArguments, CompileCallTarget, CompileCalleeTarget, CompileConstructorTarget,
    CompileConstructorValue, CompileDynamicConstructorField, CompileMemberTarget,
    CompilePatternConstructorTarget, CompilePlacedCallArgument, CompilePlacedCallValue,
    CompilePositionalPolicy, CompileTryFamily, CompileTryLayoutTarget, CompileTryTarget,
    MirGuardLocation, MirTypeContract,
};
use vela_registry::{DefinitionRegistry, FunctionDef, FunctionSignature, ParamDef};

use super::{FixtureRoots, SemanticFixture, prepare_source, prepare_source_with_registry};
use crate::compiler::error::CompileErrorKind;

#[test]
fn try_targets_follow_owning_return_contract_and_nested_body_rules() {
    let option = prepare_source(
        r#"
fn probe(value: Option<i64>) -> Option<i64> {
    let inner = value?;
    return Option::Some(inner);
}
"#,
        FixtureRoots::Program,
    )
    .expect("typed Option try input");
    assert_eq!(
        only_try_target(&option),
        CompileTryTarget::Expected(option_layout())
    );
    let option_descriptor = option
        .input
        .targets()
        .type_descriptor(option_layout().type_id)
        .expect("standard Option descriptor");
    assert_eq!(option_descriptor.canonical_name, "std::Option");
    assert_eq!(option_descriptor.runtime_name, "Option");
    assert!(option.input.targets().type_by_name("Option").is_none());

    let result = prepare_source(
        r#"
fn probe(value: Result<i64, String>) -> Result<i64, String> {
    let inner = value?;
    return Result::Ok(inner);
}
"#,
        FixtureRoots::Program,
    )
    .expect("typed Result try input");
    assert_eq!(
        only_try_target(&result),
        CompileTryTarget::Expected(result_layout())
    );

    let untyped = prepare_source(
        "fn probe(value) { let inner = value?; return inner; }",
        FixtureRoots::Program,
    )
    .expect("untyped try input");
    assert_eq!(only_try_target(&untyped), dynamic_try_target());

    let lambda = prepare_source(
        r#"
fn probe(value: Option<i64>) -> Option<i64> {
    let unwrap = || value?;
    return Option::Some(1);
}
"#,
        FixtureRoots::Program,
    )
    .expect("lambda try input");
    assert_eq!(only_try_target(&lambda), dynamic_try_target());

    let parameter_default = prepare_source(
        r#"
fn probe(value: Option<i64> = Option::Some(Option::Some(1)?)) -> Option<i64> {
    return value;
}
"#,
        FixtureRoots::Program,
    )
    .expect("parameter-default try input");
    assert_eq!(
        only_try_target(&parameter_default),
        CompileTryTarget::Expected(option_layout())
    );
}

#[test]
fn shared_trait_body_try_targets_are_scoped_by_function() {
    let fixture = prepare_source(
        r#"
trait Probe {
    fn probe(self, value: Option<i64>) -> Option<i64> {
        let inner = value?;
        return Option::Some(inner);
    }
}
struct Player {}
struct Monster {}
impl Probe for Player {}
impl Probe for Monster {}
"#,
        FixtureRoots::Program,
    )
    .expect("shared trait-default try input");
    let targets = fixture.input.targets();
    let (body, expression) = fixture.try_expressions[0];
    let functions = targets.functions_for_body(body);

    assert_eq!(
        functions.len(),
        2,
        "one shared HIR body must back both methods"
    );
    for function in functions {
        assert_eq!(
            targets
                .function_targets(*function)
                .expect("selected method root")
                .try_target(expression),
            Some(&CompileTryTarget::Expected(option_layout()))
        );
    }
    let layout = option_layout();
    assert!(targets.type_descriptor(layout.type_id).is_some());
    assert!(
        targets
            .variant_descriptor(layout.continue_variant)
            .is_some()
    );
    assert!(targets.variant_descriptor(layout.break_variant).is_some());
    assert!(targets.field_descriptor(layout.continue_payload).is_some());
}

#[test]
fn source_result_and_standard_try_layout_descriptors_coexist() {
    let fixture = prepare_source(
        r#"
enum Result {
    Ok(value)
    Err(message)
}
fn checked(value) { return Result::Ok(value); }
fn main() {
    let value = checked(10)?;
    return Result::Ok(value + 1);
}
"#,
        FixtureRoots::Program,
    )
    .expect("source Result and standard dynamic try layouts must coexist");
    let targets = fixture.input.targets();
    let script = targets
        .type_by_name("script::Result")
        .expect("package-qualified script Result descriptor");
    let standard = targets
        .type_by_name("std::Result")
        .expect("package-qualified standard Result descriptor");

    assert_eq!(script.id, vela_def::script_type_id("Result", None));
    assert_eq!(standard.id, result_layout().type_id);
    assert_ne!(script.id, standard.id);
    let expression = fixture.try_expressions[0].1;
    let placed = targets
        .compilation_roots()
        .find_map(|(function, _)| {
            targets
                .function_targets(function)
                .and_then(|targets| targets.try_target(expression))
        })
        .expect("function-scoped dynamic try target");
    assert_eq!(placed, &dynamic_try_target());
}

#[test]
fn dynamic_constructor_and_pattern_targets_preserve_hir_names_and_order() {
    let fixture = prepare_source(
        r#"
fn main(value) {
    let record = Missing { second: 2, first: 1 };
    let variant = Missing::Ready { label: "ready", amount: 3 };
    return match value {
        Missing::Ready { label, amount } => amount,
        Missing { second, first } => first,
        _ => 0,
    };
}
"#,
        FixtureRoots::Program,
    )
    .expect("dynamic constructor placement input");
    let targets = fixture.input.targets();
    let function = targets.compilation_roots().next().expect("main root").0;
    let scoped = targets.function_targets(function).expect("main targets");

    for (_, expression, path) in &fixture.constructor_expressions {
        let expected = match path.as_slice() {
            [name] => CompileConstructorTarget::DynamicRecord {
                type_name: name.clone(),
                fields: vec![
                    dynamic_field("second", scoped, *expression, 0),
                    dynamic_field("first", scoped, *expression, 1),
                ],
            },
            [owner, variant] => CompileConstructorTarget::DynamicVariant {
                owner_name: owner.clone(),
                variant_name: variant.clone(),
                fields: vec![
                    dynamic_field("label", scoped, *expression, 0),
                    dynamic_field("amount", scoped, *expression, 1),
                ],
            },
            other => panic!("unexpected dynamic constructor path {other:?}"),
        };
        assert_eq!(scoped.constructor(*expression), Some(&expected));
    }

    for (_, pattern, path) in &fixture.constructor_patterns {
        let expected = match path.as_slice() {
            [name] => CompilePatternConstructorTarget::DynamicRecord {
                type_name: name.clone(),
                fields: vec!["second".to_owned(), "first".to_owned()],
            },
            [owner, variant] => CompilePatternConstructorTarget::DynamicVariant {
                owner_name: owner.clone(),
                variant_name: variant.clone(),
                fields: vec!["label".to_owned(), "amount".to_owned()],
            },
            other => panic!("unexpected dynamic pattern path {other:?}"),
        };
        assert_eq!(scoped.pattern_constructor(*pattern), Some(&expected));
    }
}

#[test]
fn dynamic_constructor_duplicates_keep_the_frozen_diagnostic() {
    let error = prepare_source(
        "fn main() { return Missing { amount: 1, amount: 2 }; }",
        FixtureRoots::Program,
    )
    .expect_err("duplicate dynamic fields must fail before target insertion");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics, got {error:?}");
    };

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_deref() == Some("compiler::duplicate_constructor_field")
    }));
}

#[test]
fn named_script_and_external_calls_retain_source_order_and_parameter_slots() {
    let script = prepare_source(
        r#"
fn target(first, second, third = 3) { return first + second + third; }
fn main() { return target(second = 2, first = 1); }
"#,
        FixtureRoots::Program,
    )
    .expect("named script call placement");
    let CompileCallArguments::Script {
        evaluation_order,
        parameter_slots,
    } = &only_call_target(&script).arguments
    else {
        panic!("expected placed script arguments");
    };
    assert_named_call_order(evaluation_order, parameter_slots);

    let mut registry = DefinitionRegistry::new();
    registry
        .register_function(FunctionDef::new(
            DefPath::function("host", ["audit"], "send"),
            FunctionSignature::new(
                [
                    ParamDef::new("first", Some("i64")),
                    ParamDef::new("second", Some("i64")),
                    ParamDef::new("third", Some("i64")).defaulted(true),
                ],
                Some("i64"),
            ),
        ))
        .expect("external function fixture");
    let external = prepare_source_with_registry(
        "fn main() { return audit::send(second = 2, first = 1); }",
        FixtureRoots::Program,
        registry.compile_view(),
    )
    .expect("named external call placement");
    let CompileCallArguments::ExternalNamed {
        evaluation_order,
        parameter_slots,
    } = &only_call_target(&external).arguments
    else {
        panic!("expected placed external arguments");
    };
    assert_named_call_order(evaluation_order, parameter_slots);
}

#[test]
fn named_script_argument_guards_retain_callee_parameter_indices() {
    let fixture = prepare_source(
        r#"
fn target(first: i64, second: String) {}
fn main(value) { target(second = value, first = value); }
"#,
        FixtureRoots::Program,
    )
    .expect("named script argument guard input");

    for (parameter, contract, name) in [
        (0, MirTypeContract::Primitive(PrimitiveTag::I64), "first"),
        (
            1,
            MirTypeContract::Primitive(PrimitiveTag::String),
            "second",
        ),
    ] {
        let expression =
            call_argument_expression(&fixture, "target(second = value, first = value)", parameter);
        let guard = expression_guard(&fixture, expression)
            .unwrap_or_else(|| panic!("missing script parameter guard {parameter}"));
        assert_eq!(guard.contract, contract);
        assert_eq!(
            guard.context.location,
            MirGuardLocation::Parameter {
                index: u32::try_from(parameter).expect("fixture parameter index")
            }
        );
        assert_eq!(guard.context.debug_name, name);
    }
}

#[test]
fn named_record_and_tuple_constructors_retain_source_order_and_schema_slots() {
    let record = prepare_source(
        r#"
struct Pair { first: i64, second: i64, third: i64 = 3 }
fn main() { return Pair { second: 2, first: 1 }; }
"#,
        FixtureRoots::Program,
    )
    .expect("named record constructor placement");
    assert_named_constructor_order(only_constructor_target(&record));

    let tuple = prepare_source(
        r#"
enum Pair { Values(first: i64, second: i64, third: i64 = 3) }
fn main() { return Pair::Values(second = 2, first = 1); }
"#,
        FixtureRoots::Program,
    )
    .expect("named tuple constructor placement");
    assert_named_constructor_order(only_constructor_target(&tuple));
}

#[test]
fn policy_controlled_reflection_calls_use_the_authoritative_neutral_manifest() {
    let positional = prepare_source(
        "fn main() { return reflect::methods(1); }",
        FixtureRoots::Program,
    )
    .expect("the reflection manifest closes positional placement");
    let call = only_call_target(&positional);
    let CompileCalleeTarget::NativeFunction { function, .. } = call.callee else {
        panic!("reflect::methods must remain a runtime native");
    };
    assert!(matches!(
        call.arguments,
        CompileCallArguments::Positional(ref arguments) if arguments.len() == 1
    ));
    let descriptor = positional
        .input
        .targets()
        .function_descriptor(function)
        .expect("runtime native descriptor");
    assert_eq!(descriptor.signature.parameters.len(), 1);
    assert_eq!(descriptor.signature.parameters[0].name, "target");
    assert_eq!(
        descriptor.signature.positional,
        CompilePositionalPolicy::RuntimeChecked
    );
    assert!(descriptor.signature.effect.reflection_read);

    let named = prepare_source(
        "fn main() { return reflect::methods(target = 1); }",
        FixtureRoots::Program,
    )
    .expect("the manifest parameter name authorizes named placement");
    assert!(matches!(
        only_call_target(&named).arguments,
        CompileCallArguments::ExternalNamed {
            ref evaluation_order,
            ref parameter_slots,
        } if evaluation_order.len() == 1 && parameter_slots.len() == 1
    ));

    let error = prepare_source(
        "fn main() { return reflect::methods(value = 1); }",
        FixtureRoots::Program,
    )
    .expect_err("unknown reflection parameter names remain source diagnostics");
    assert!(matches!(
        error.kind,
        CompileErrorKind::SemanticDiagnostics(ref diagnostics)
            if diagnostics.iter().any(|diagnostic| {
                diagnostic.code.as_deref() == Some("compiler::unknown_named_argument")
                    && diagnostic.message.contains("value")
            })
    ));
}

#[test]
fn closed_script_member_miss_remains_an_explicit_dynamic_lookup() {
    let fixture = prepare_source(
        r#"
struct Reward { amount: i64 }
fn main() {
    let reward = Reward { amount: 1 };
    return reward.missing;
}
"#,
        FixtureRoots::Program,
    )
    .expect("a closed member miss remains a runtime lookup");
    let (body, expression, _) = fixture
        .member_expressions
        .iter()
        .find(|(_, _, name)| name == "missing")
        .expect("missing member expression");
    let (function, _) = fixture
        .input
        .targets()
        .compilation_roots()
        .find(|(_, root)| root.body == *body)
        .expect("owning executable root");

    assert_eq!(
        fixture
            .input
            .targets()
            .function_targets(function)
            .expect("scoped targets")
            .member(*expression),
        Some(&CompileMemberTarget::Dynamic {
            name: "missing".to_owned(),
        })
    );
}

#[test]
fn set_from_array_calls_retain_one_canonical_positional_operand() {
    for (source, operand) in [
        ("fn main() { return set::from_array([1, 2]); }", "[1, 2]"),
        (
            "fn main() { return set::from_array(values = [3, 4]); }",
            "[3, 4]",
        ),
    ] {
        let fixture = prepare_source(source, FixtureRoots::Program)
            .expect("set::from_array placement must be complete");
        let target = only_call_target(&fixture);
        assert!(matches!(
            &target.callee,
            CompileCalleeTarget::SetFromArray { debug_name, .. }
                if debug_name == "set::from_array"
        ));
        assert_eq!(
            target.arguments,
            CompileCallArguments::Positional(vec![expression_exact(&fixture, operand)])
        );
    }
}

#[test]
fn typed_container_mutations_contextualize_scalar_arguments() {
    let fixture = prepare_source(
        r#"
fn main(array: Array<u8>, map: Map<u16, i32>, set: Set<u64>) {
    array.push(1);
    array.insert(0, 2);
    map.set(3, 4);
    set.add(5);
}
"#,
        FixtureRoots::Program,
    )
    .expect("typed mutation scalar contexts");

    let cases = [
        ("array.push(1)", 0, ScalarValue::U8(1)),
        ("array.insert(0, 2)", 0, ScalarValue::I64(0)),
        ("array.insert(0, 2)", 1, ScalarValue::U8(2)),
        ("map.set(3, 4)", 0, ScalarValue::U16(3)),
        ("map.set(3, 4)", 1, ScalarValue::I32(4)),
        ("set.add(5)", 0, ScalarValue::U64(5)),
    ];
    for (call, parameter, expected) in cases {
        let expression = call_argument_expression(&fixture, call, parameter);
        assert_eq!(
            resolved_scalar(&fixture, expression),
            Some(expected),
            "{call}"
        );
    }
}

#[test]
fn typed_container_mutations_reject_static_argument_mismatches() {
    const SOURCE: &str = r#"
fn main(values: Array<u8>) {
    values.push("bad");
}
"#;
    let error = prepare_source(SOURCE, FixtureRoots::Program)
        .expect_err("typed array mutation must reject a static mismatch");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics, got {error:?}");
    };
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("compiler::type_contract_mismatch"))
        .expect("typed mutation mismatch diagnostic");
    let span = diagnostic.span.expect("mismatched argument span");
    assert_eq!(&SOURCE[span.start as usize..span.end as usize], "\"bad\"");
    assert_eq!(
        diagnostic.message,
        "type contract mismatch for native parameter `push::value`"
    );
    assert_eq!(diagnostic.labels.len(), 1);
    assert_eq!(
        diagnostic.labels[0].message,
        "expected `u8`, found `String`"
    );
}

#[test]
fn typed_container_mutations_own_all_dynamic_argument_guards() {
    let fixture = prepare_source(
        r#"
fn main(array: Array<u8>, map: Map<u16, i32>, set: Set<u64>, value) {
    array.push(value);
    array.insert(0, value);
    array.extend(value);
    map.set(value, value);
    map.extend(value);
    set.add(value);
    set.extend(value);
}
"#,
        FixtureRoots::Program,
    )
    .expect("typed container mutation guard input");

    let cases = [
        (
            "array.push(value)",
            0,
            MirTypeContract::Primitive(PrimitiveTag::U8),
            "value",
        ),
        (
            "array.insert(0, value)",
            1,
            MirTypeContract::Primitive(PrimitiveTag::U8),
            "value",
        ),
        (
            "array.extend(value)",
            0,
            MirTypeContract::Array(Some(Box::new(MirTypeContract::Primitive(PrimitiveTag::U8)))),
            "values",
        ),
        (
            "map.set(value, value)",
            0,
            MirTypeContract::Primitive(PrimitiveTag::U16),
            "key",
        ),
        (
            "map.set(value, value)",
            1,
            MirTypeContract::Primitive(PrimitiveTag::I32),
            "value",
        ),
        (
            "map.extend(value)",
            0,
            MirTypeContract::Map {
                key: Some(Box::new(MirTypeContract::Primitive(PrimitiveTag::U16))),
                value: Some(Box::new(MirTypeContract::Primitive(PrimitiveTag::I32))),
            },
            "values",
        ),
        (
            "set.add(value)",
            0,
            MirTypeContract::Primitive(PrimitiveTag::U64),
            "value",
        ),
        (
            "set.extend(value)",
            0,
            MirTypeContract::Set(Some(Box::new(MirTypeContract::Primitive(
                PrimitiveTag::U64,
            )))),
            "values",
        ),
    ];
    for (call, parameter, contract, debug_name) in cases {
        let expression = call_argument_expression(&fixture, call, parameter);
        let guard = expression_guard(&fixture, expression).unwrap_or_else(|| {
            panic!("missing typed mutation guard for {call} parameter {parameter}")
        });
        assert_eq!(guard.contract, contract, "{call} parameter {parameter}");
        assert_eq!(
            guard.context.location,
            MirGuardLocation::Parameter {
                index: u32::try_from(parameter).expect("fixture parameter index")
            },
            "{call} parameter {parameter}"
        );
        assert_eq!(
            guard.context.debug_name, debug_name,
            "{call} parameter {parameter}"
        );
    }
}

#[test]
fn erased_container_receivers_keep_only_manifest_level_contracts() {
    let fixture = prepare_source(
        r#"
fn main(
    array: Array,
    any_array: Array<Any>,
    map: Map,
    partial_map: Map<Any, i32>,
    partial_key_map: Map<u16, Any>,
    string_map: Map<String, i32>,
    set: Set,
    value,
) {
    let inferred_but_erased: Array = [1i32];
    array.push(value);
    any_array.push(value);
    inferred_but_erased.push(value);
    map.set(value, value);
    partial_map.set(value, value);
    partial_key_map.set(value, value);
    string_map.set(value, value);
    set.add(value);
    array.extend(value);
    any_array.extend(value);
    map.extend(value);
    partial_map.extend(value);
    set.extend(value);
}
"#,
        FixtureRoots::Program,
    )
    .expect("erased container mutation contracts");

    for (call, parameters) in [
        ("array.push(value)", &[0][..]),
        ("any_array.push(value)", &[0][..]),
        ("inferred_but_erased.push(value)", &[0][..]),
        ("map.set(value, value)", &[0, 1][..]),
        ("partial_map.set(value, value)", &[0, 1][..]),
        ("partial_key_map.set(value, value)", &[0, 1][..]),
        ("set.add(value)", &[0][..]),
    ] {
        for parameter in parameters {
            let expression = call_argument_expression(&fixture, call, *parameter);
            assert!(
                expression_guard(&fixture, expression).is_none(),
                "erased receiver invented a typed guard for {call} parameter {parameter}"
            );
        }
    }

    let string_key = call_argument_expression(&fixture, "string_map.set(value, value)", 0);
    assert!(
        expression_guard(&fixture, string_key).is_none(),
        "Map<String, _>.set preserves the existing unguarded key contract"
    );
    let string_value = call_argument_expression(&fixture, "string_map.set(value, value)", 1);
    assert_eq!(
        expression_guard(&fixture, string_value).map(|guard| &guard.contract),
        Some(&MirTypeContract::Primitive(PrimitiveTag::I32))
    );

    for (call, expected) in [
        ("array.extend(value)", MirTypeContract::Array(None)),
        ("any_array.extend(value)", MirTypeContract::Array(None)),
        (
            "map.extend(value)",
            MirTypeContract::Map {
                key: None,
                value: None,
            },
        ),
        (
            "partial_map.extend(value)",
            MirTypeContract::Map {
                key: None,
                value: None,
            },
        ),
        ("set.extend(value)", MirTypeContract::Set(None)),
    ] {
        let expression = call_argument_expression(&fixture, call, 0);
        assert_eq!(
            expression_guard(&fixture, expression).map(|guard| &guard.contract),
            Some(&expected),
            "{call} must retain its manifest-level container guard"
        );
    }
}

#[test]
fn neutral_record_descriptor_and_constructor_keep_declaration_order() {
    let fixture = prepare_source(
        r#"
struct Layout { zeta: i64, alpha: i64 }
fn main() { return Layout { alpha: 1, zeta: 2 }; }
"#,
        FixtureRoots::Program,
    )
    .expect("non-alphabetic record declaration order");
    let targets = fixture.input.targets();
    let descriptor = targets
        .type_by_name("script::Layout")
        .expect("script record descriptor");
    let names = descriptor
        .fields
        .iter()
        .map(|field| {
            targets
                .field_descriptor(*field)
                .expect("record field descriptor")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(names, ["zeta", "alpha"]);

    let CompileConstructorTarget::Record {
        evaluation_order,
        fields,
        ..
    } = only_constructor_target(&fixture)
    else {
        panic!("expected static record constructor");
    };
    assert_eq!(evaluation_order.len(), 2);
    assert_eq!(
        fields.iter().map(|field| field.field).collect::<Vec<_>>(),
        descriptor.fields.as_slice()
    );
    assert_eq!(
        fields[0].value,
        CompileConstructorValue::Explicit {
            source_index: 1,
            value: evaluation_order[1],
        }
    );
    assert_eq!(
        fields[1].value,
        CompileConstructorValue::Explicit {
            source_index: 0,
            value: evaluation_order[0],
        }
    );
}

fn assert_named_call_order(
    evaluation_order: &[vela_hir::ids::HirExprId],
    slots: &[CompilePlacedCallArgument],
) {
    assert_eq!(evaluation_order.len(), 2);
    assert_eq!(slots.len(), 3);
    assert_eq!(slots[0].parameter, 0);
    assert_eq!(
        slots[0].value,
        CompilePlacedCallValue::Explicit {
            source_index: 1,
            value: evaluation_order[1],
        }
    );
    assert_eq!(slots[1].parameter, 1);
    assert_eq!(
        slots[1].value,
        CompilePlacedCallValue::Explicit {
            source_index: 0,
            value: evaluation_order[0],
        }
    );
    assert_eq!(slots[2].parameter, 2);
    assert_eq!(slots[2].value, CompilePlacedCallValue::MissingDefault);
}

fn assert_named_constructor_order(target: &CompileConstructorTarget) {
    let (evaluation_order, fields) = match target {
        CompileConstructorTarget::Record {
            evaluation_order,
            fields,
            ..
        }
        | CompileConstructorTarget::Variant {
            evaluation_order,
            fields,
            ..
        } => (evaluation_order, fields),
        CompileConstructorTarget::DynamicRecord { .. }
        | CompileConstructorTarget::DynamicVariant { .. } => {
            panic!("expected static constructor placement")
        }
    };
    assert_eq!(evaluation_order.len(), 2);
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].parameter, 0);
    assert_eq!(
        fields[0].value,
        CompileConstructorValue::Explicit {
            source_index: 1,
            value: evaluation_order[1],
        }
    );
    assert_eq!(fields[1].parameter, 1);
    assert_eq!(
        fields[1].value,
        CompileConstructorValue::Explicit {
            source_index: 0,
            value: evaluation_order[0],
        }
    );
    assert_eq!(fields[2].parameter, 2);
    assert!(matches!(
        fields[2].value,
        CompileConstructorValue::EvaluatedDefault(_)
    ));
}

fn only_call_target(fixture: &SemanticFixture) -> &CompileCallTarget {
    assert_eq!(fixture.call_expressions.len(), 1);
    let expression = fixture.call_expressions[0].1;
    let targets = fixture.input.targets();
    targets
        .compilation_roots()
        .find_map(|(function, _)| targets.function_targets(function)?.call(expression))
        .expect("call target placement")
}

fn call_argument_expression(
    fixture: &SemanticFixture,
    call_source: &str,
    parameter: usize,
) -> vela_hir::ids::HirExprId {
    let call = expression_exact(fixture, call_source);
    let target = fixture
        .input
        .targets()
        .compilation_roots()
        .find_map(|(function, _)| {
            fixture
                .input
                .targets()
                .function_targets(function)?
                .call(call)
        })
        .unwrap_or_else(|| panic!("missing call target for `{call_source}`"));
    match &target.arguments {
        CompileCallArguments::Positional(values) => values.get(parameter).copied(),
        CompileCallArguments::ExternalNamed {
            parameter_slots, ..
        }
        | CompileCallArguments::Script {
            parameter_slots, ..
        } => parameter_slots
            .get(parameter)
            .and_then(|slot| match slot.value {
                CompilePlacedCallValue::Explicit { value, .. } => Some(value),
                CompilePlacedCallValue::MissingDefault => None,
            }),
        CompileCallArguments::Dynamic(_) => None,
    }
    .unwrap_or_else(|| panic!("missing parameter {parameter} for `{call_source}`"))
}

fn expression_guard(
    fixture: &SemanticFixture,
    expression: vela_hir::ids::HirExprId,
) -> Option<&vela_mir::CompileGuardTarget> {
    fixture
        .input
        .targets()
        .compilation_roots()
        .find_map(|(function, _)| {
            fixture
                .input
                .targets()
                .function_targets(function)?
                .expression_guard(expression)
        })
}

fn resolved_scalar(
    fixture: &SemanticFixture,
    expression: vela_hir::ids::HirExprId,
) -> Option<ScalarValue> {
    fixture.input.analysis().functions().find_map(|function| {
        fixture
            .input
            .analysis()
            .view(function)?
            .literal(expression)?
            .as_ref()
            .ok()?
            .scalar()
    })
}

fn expression_exact(fixture: &SemanticFixture, source: &str) -> vela_hir::ids::HirExprId {
    let matches = fixture
        .expression_sources
        .iter()
        .filter_map(|(_, expression, text)| (text == source).then_some(*expression))
        .collect::<Vec<_>>();
    let [expression] = matches.as_slice() else {
        panic!("expected one expression `{source}`, got {matches:?}");
    };
    *expression
}

fn only_constructor_target(fixture: &SemanticFixture) -> &CompileConstructorTarget {
    let expression = fixture
        .constructor_expressions
        .first()
        .map(|(_, expression, _)| *expression)
        .or_else(|| {
            fixture
                .call_expressions
                .first()
                .map(|(_, expression)| *expression)
        })
        .expect("constructor expression");
    let targets = fixture.input.targets();
    targets
        .compilation_roots()
        .find_map(|(function, _)| targets.function_targets(function)?.constructor(expression))
        .expect("constructor target placement")
}

fn only_try_target(fixture: &SemanticFixture) -> CompileTryTarget {
    assert_eq!(fixture.try_expressions.len(), 1);
    let expression = fixture.try_expressions[0].1;
    let targets = fixture.input.targets();
    let function = targets
        .compilation_roots()
        .next()
        .expect("single fixture root")
        .0;
    *targets
        .function_targets(function)
        .expect("selected root targets")
        .try_target(expression)
        .expect("try placement")
}

fn dynamic_field(
    name: &str,
    targets: vela_mir::CompileFunctionTargets<'_>,
    expression: vela_hir::ids::HirExprId,
    index: usize,
) -> CompileDynamicConstructorField {
    let target = targets
        .constructor(expression)
        .expect("dynamic constructor target");
    let fields = match target {
        CompileConstructorTarget::DynamicRecord { fields, .. }
        | CompileConstructorTarget::DynamicVariant { fields, .. } => fields,
        CompileConstructorTarget::Record { .. } | CompileConstructorTarget::Variant { .. } => {
            panic!("expected dynamic constructor")
        }
    };
    CompileDynamicConstructorField {
        name: name.to_owned(),
        value: fields[index].value,
    }
}

fn dynamic_try_target() -> CompileTryTarget {
    CompileTryTarget::Dynamic {
        option: option_layout(),
        result: result_layout(),
    }
}

fn option_layout() -> CompileTryLayoutTarget {
    CompileTryLayoutTarget {
        family: CompileTryFamily::Option,
        type_id: vela_stdlib::std_type_id("Option").expect("Option type ID"),
        continue_variant: vela_stdlib::std_variant_id("Option", "Some")
            .expect("Option::Some variant ID"),
        break_variant: vela_stdlib::std_variant_id("Option", "None")
            .expect("Option::None variant ID"),
        continue_payload: vela_stdlib::std_field_id("Option::Some", "0")
            .expect("Option::Some payload ID"),
    }
}

fn result_layout() -> CompileTryLayoutTarget {
    CompileTryLayoutTarget {
        family: CompileTryFamily::Result,
        type_id: vela_stdlib::std_type_id("Result").expect("Result type ID"),
        continue_variant: vela_stdlib::std_variant_id("Result", "Ok")
            .expect("Result::Ok variant ID"),
        break_variant: vela_stdlib::std_variant_id("Result", "Err")
            .expect("Result::Err variant ID"),
        continue_payload: vela_stdlib::std_field_id("Result::Ok", "0")
            .expect("Result::Ok payload ID"),
    }
}
