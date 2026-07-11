use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_analysis::literals::LiteralPrimitiveContext;
use vela_common::{PrimitiveTag, ShapeId, SourceId};
use vela_def::{FieldId, FunctionId, TypeId, VariantId};
use vela_hir::body::{HirBody, HirExprKind};
use vela_hir::ids::{HirBodyId, HirExprId};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use super::*;
use crate::{
    CompileFieldAccess, CompileFieldDescriptor, CompileFunctionAccess, CompileFunctionClass,
    CompileFunctionDescriptor, CompileFunctionIdentity, CompileGuardKey, CompileGuardTarget,
    CompilePositionalPolicy, CompileSignature, CompileTargetSnapshot, CompileTargetSnapshotBuilder,
    CompileTypeClass, CompileTypeDescriptor, CompileVariantDescriptor, MirGuardLocation,
    MirLoweringConfig, MirLoweringInput, MirProgram, MirTypeContract,
};

const ROOT_FUNCTION: FunctionId = FunctionId::new(8_500);
const RECORD_TYPE: TypeId = TypeId::new(8_501);
const RECORD_SHAPE: ShapeId = ShapeId::new(8_502);
const ENUM_TYPE: TypeId = TypeId::new(8_503);
const VARIANT: VariantId = VariantId::new(8_504);

fn lower(
    source: &str,
    expression_text: &str,
    literal_contexts: &[(&str, LiteralPrimitiveContext)],
    configure: impl FnOnce(
        &HirBody,
        HirExprId,
        &mut CompileTargetSnapshotBuilder,
        MirSourceOrigin,
    ) -> Result<(), MirBuildError>,
) -> Result<MirProgram, MirBuildError> {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(85),
        ModulePath::from_qualified("constructors"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let body = graph.function_body(declaration.id).expect("main HIR body");
    let expression = expression_by_text(body, source, expression_text);
    let contexts = literal_contexts
        .iter()
        .map(|(text, context)| (expression_by_text(body, source, text), *context));
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(ROOT_FUNCTION, body.id).with_literal_contexts(contexts)],
    )
    .expect("constructor analysis generation");

    let body_origin = MirSourceOrigin::body(body.id, body.origin.span);
    let expression_origin = expression_origin(body, expression);
    let mut targets = CompileTargetSnapshot::builder();
    targets.insert_script_function(
        declaration.id,
        body.id,
        CompileFunctionDescriptor {
            id: ROOT_FUNCTION,
            class: CompileFunctionClass::Script,
            canonical_symbol: "constructors::main".to_owned(),
            debug_name: "main".to_owned(),
            signature: CompileSignature {
                parameters: Vec::new(),
                positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                return_contract: None,
                effect: MirEffect::PURE,
            },
            access: CompileFunctionAccess::script(false),
        },
        body_origin,
    )?;
    configure(body, expression, &mut targets, expression_origin)?;
    let targets = targets.build()?;
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(ROOT_FUNCTION),
        body.id,
        analysis.view(ROOT_FUNCTION).expect("root analysis"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: false,
            compute_liveness: false,
        },
    )?;

    crate::build_mir(input)
}

fn expression_by_text(body: &HirBody, source: &str, text: &str) -> HirExprId {
    let matches = source
        .match_indices(text)
        .flat_map(|(start, _)| {
            let end = start + text.len();
            body.expressions.values().filter_map(move |expression| {
                (expression.origin.span.start == u32::try_from(start).expect("fixture start")
                    && expression.origin.span.end == u32::try_from(end).expect("fixture end"))
                .then_some(expression.id)
            })
        })
        .collect::<Vec<_>>();
    let [expression] = matches.as_slice() else {
        panic!("expected one HIR expression for {text:?}, got {matches:?}")
    };
    *expression
}

fn expression_origin(body: &HirBody, expression: HirExprId) -> MirSourceOrigin {
    let expression = body.expression(expression).expect("fixture expression");
    MirSourceOrigin::expression(body.id, expression.id, expression.origin.span)
}

fn record_values(body: &HirBody, expression: HirExprId) -> Vec<(String, HirExprId)> {
    let HirExprKind::Record { fields, .. } = &body
        .expression(expression)
        .expect("record constructor expression")
        .kind
    else {
        panic!("expected record constructor expression")
    };
    fields
        .iter()
        .map(|field| {
            (
                field.name.clone(),
                field.value.expect("constructor field value"),
            )
        })
        .collect()
}

fn call_values(body: &HirBody, expression: HirExprId) -> Vec<HirExprId> {
    body.call(expression)
        .expect("tuple variant call")
        .arguments
        .iter()
        .map(|argument| argument.value.expect("variant argument value"))
        .collect()
}

fn insert_record_layout(
    targets: &mut CompileTargetSnapshotBuilder,
    fields: &[(FieldId, &str, Option<MirTypeContract>)],
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    targets.insert_type_descriptor(
        CompileTypeDescriptor {
            id: RECORD_TYPE,
            canonical_name: "constructors::Record".to_owned(),
            class: CompileTypeClass::ScriptRecord,
            shape: Some(RECORD_SHAPE),
            fields: fields.iter().map(|(field, _, _)| *field).collect(),
            variants: Vec::new(),
        },
        origin,
    )?;
    for (index, (field, name, contract)) in fields.iter().enumerate() {
        targets.insert_field_descriptor(
            CompileFieldDescriptor {
                id: *field,
                owner: RECORD_TYPE,
                variant: None,
                name: (*name).to_owned(),
                contract: contract.clone(),
                declaration_order: u32::try_from(index).expect("field index"),
                access: CompileFieldAccess::script(),
                host_runtime: None,
            },
            origin,
        )?;
        if let Some(contract) = contract {
            targets.insert_guard(
                CompileGuardKey::Field(*field),
                CompileGuardTarget::new(contract.clone(), MirGuardLocation::Field, *name),
                origin,
            )?;
        }
    }
    Ok(())
}

fn only_function(program: &MirProgram) -> &crate::MirFunction {
    let functions = program.functions().collect::<Vec<_>>();
    let [(_, function)] = functions.as_slice() else {
        panic!("expected one MIR function, got {}", functions.len())
    };
    function
}

#[test]
fn constructor_builder_evaluates_named_fields_then_projects_slots_and_defaults() {
    let first = FieldId::new(8_510);
    let second = FieldId::new(8_511);
    let fallback = FieldId::new(8_512);
    let default_body = HirBodyId::new(8_513);
    let program = lower(
        r#"fn main() { return Package { second: "second", first: "first" }; }"#,
        r#"Package { second: "second", first: "first" }"#,
        &[],
        |body, expression, targets, origin| {
            insert_record_layout(
                targets,
                &[
                    (first, "first", None),
                    (second, "second", None),
                    (fallback, "fallback", None),
                ],
                origin,
            )?;
            let values = record_values(body, expression);
            let second_value = values[0].1;
            let first_value = values[1].1;
            targets.insert_evaluated_schema_default(
                default_body,
                MirEvaluatedConstant::String("default".to_owned()),
                origin,
            )?;
            targets.insert_constructor(
                ROOT_FUNCTION,
                expression,
                CompileConstructorTarget::Record {
                    type_id: RECORD_TYPE,
                    shape: RECORD_SHAPE,
                    evaluation_order: vec![second_value, first_value],
                    fields: vec![
                        CompileConstructorField {
                            field: first,
                            parameter: 0,
                            parameter_name: "first".to_owned(),
                            value: CompileConstructorValue::Explicit {
                                source_index: 1,
                                value: first_value,
                            },
                        },
                        CompileConstructorField {
                            field: second,
                            parameter: 1,
                            parameter_name: "second".to_owned(),
                            value: CompileConstructorValue::Explicit {
                                source_index: 0,
                                value: second_value,
                            },
                        },
                        CompileConstructorField {
                            field: fallback,
                            parameter: 2,
                            parameter_name: "fallback".to_owned(),
                            value: CompileConstructorValue::EvaluatedDefault(default_body),
                        },
                    ],
                },
                origin,
            )
        },
    )
    .expect("static record constructor");

    let function = only_function(&program);
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    assert!(matches!(
        &statements[0].kind,
        MirStatementKind::MaterializeConstant(MirEvaluatedConstant::String(value)) if value == "second"
    ));
    assert!(matches!(
        &statements[1].kind,
        MirStatementKind::MaterializeConstant(MirEvaluatedConstant::String(value)) if value == "first"
    ));
    assert!(matches!(
        &statements[2].kind,
        MirStatementKind::MaterializeConstant(MirEvaluatedConstant::String(value)) if value == "default"
    ));
    let MirStatementKind::Allocate(MirAggregate::Record {
        type_id,
        shape,
        fields,
    }) = &statements[3].kind
    else {
        panic!("expected record allocation, got {:?}", statements[3].kind)
    };
    assert_eq!((*type_id, *shape), (RECORD_TYPE, RECORD_SHAPE));
    assert_eq!(
        fields.iter().map(|(field, _)| *field).collect::<Vec<_>>(),
        vec![first, second, fallback]
    );
    assert!(
        matches!(fields[0].1, MirOperand::Temp(temp) if Some(MirPlace::temp(temp)) == statements[1].destination)
    );
    assert!(
        matches!(fields[1].1, MirOperand::Temp(temp) if Some(MirPlace::temp(temp)) == statements[0].destination)
    );
    assert!(
        matches!(fields[2].1, MirOperand::Temp(temp) if Some(MirPlace::temp(temp)) == statements[2].destination)
    );
    let allocation = statements[3].destination.expect("allocation destination");
    let MirPlace::Temp(allocation) = allocation else {
        panic!("constructor allocation must define a temp")
    };
    assert_eq!(
        function
            .temp(allocation)
            .expect("allocation temp")
            .value_type,
        MirValueType::ScriptType {
            type_id: RECORD_TYPE,
            shape: RECORD_SHAPE,
        }
    );
    assert_eq!(function.safepoints().count(), 4);
}

#[test]
fn constructor_builder_uses_only_the_authoritative_expression_guard() {
    let field = FieldId::new(8_520);
    let source = r#"fn main() { return Small { value: if true { 1 } else { "x" } }; }"#;
    let expression_text = r#"Small { value: if true { 1 } else { "x" } }"#;

    for expression_guard in [true, false] {
        let program = lower(
            source,
            expression_text,
            &[],
            |body, expression, targets, origin| {
                insert_record_layout(
                    targets,
                    &[(
                        field,
                        "value",
                        Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
                    )],
                    origin,
                )?;
                let value = record_values(body, expression)[0].1;
                if expression_guard {
                    targets.insert_guard(
                        CompileGuardKey::Expression {
                            function: ROOT_FUNCTION,
                            expression: value,
                        },
                        CompileGuardTarget::new(
                            MirTypeContract::Primitive(PrimitiveTag::I64),
                            MirGuardLocation::Field,
                            "value",
                        ),
                        expression_origin(body, value),
                    )?;
                }
                targets.insert_constructor(
                    ROOT_FUNCTION,
                    expression,
                    CompileConstructorTarget::Record {
                        type_id: RECORD_TYPE,
                        shape: RECORD_SHAPE,
                        evaluation_order: vec![value],
                        fields: vec![CompileConstructorField {
                            field,
                            parameter: 0,
                            parameter_name: "value".to_owned(),
                            value: CompileConstructorValue::Explicit {
                                source_index: 0,
                                value,
                            },
                        }],
                    },
                    origin,
                )
            },
        )
        .expect("dynamic constructor field");

        let function = only_function(&program);
        let guards = function
            .guards()
            .map(|(_, guard)| guard)
            .collect::<Vec<_>>();
        if expression_guard {
            let [guard] = guards.as_slice() else {
                panic!("expected one expression guard, got {guards:?}")
            };
            assert_eq!(
                guard.assumption,
                MirGuardAssumption::Type(MirTypeContract::Primitive(PrimitiveTag::I64))
            );
            assert_eq!(
                guard.context,
                Some(crate::MirGuardContext::new(
                    MirGuardLocation::Field,
                    "value"
                ))
            );
        } else {
            assert!(
                guards.is_empty(),
                "the field declaration guard is metadata, not an RHS decision"
            );
        }
        assert_eq!(
            function
                .statements()
                .filter(|(_, statement)| matches!(
                    statement.kind,
                    MirStatementKind::GuardTrap { .. }
                ))
                .count(),
            usize::from(expression_guard)
        );
    }
}

#[test]
fn constructor_builder_does_not_reapply_field_guard_to_contextual_literal() {
    let field = FieldId::new(8_530);
    let source = "fn main() { return Small { value: 255 }; }";
    let program = lower(
        source,
        "Small { value: 255 }",
        &[("255", LiteralPrimitiveContext::Expected(PrimitiveTag::U8))],
        |body, expression, targets, origin| {
            insert_record_layout(
                targets,
                &[(
                    field,
                    "value",
                    Some(MirTypeContract::Primitive(PrimitiveTag::U8)),
                )],
                origin,
            )?;
            let value = record_values(body, expression)[0].1;
            targets.insert_constructor(
                ROOT_FUNCTION,
                expression,
                CompileConstructorTarget::Record {
                    type_id: RECORD_TYPE,
                    shape: RECORD_SHAPE,
                    evaluation_order: vec![value],
                    fields: vec![CompileConstructorField {
                        field,
                        parameter: 0,
                        parameter_name: "value".to_owned(),
                        value: CompileConstructorValue::Explicit {
                            source_index: 0,
                            value,
                        },
                    }],
                },
                origin,
            )
        },
    )
    .expect("contextual record field");

    let function = only_function(&program);
    assert_eq!(function.guards().count(), 0);
    let allocation = function
        .statements()
        .find_map(|(_, statement)| match &statement.kind {
            MirStatementKind::Allocate(MirAggregate::Record { fields, .. }) => Some(fields),
            _ => None,
        })
        .expect("record allocation");
    assert!(matches!(
        allocation.as_slice(),
        [(actual, MirOperand::Immediate(MirImmediate::Scalar(vela_common::ScalarValue::U8(255))))]
            if *actual == field
    ));
}

#[test]
fn constructor_builder_lowers_tuple_variants_with_enum_result_type() {
    let left = FieldId::new(8_540);
    let right = FieldId::new(8_541);
    let source = r#"fn main() { return Choice::Pair("left", "right"); }"#;
    let program = lower(
        source,
        r#"Choice::Pair("left", "right")"#,
        &[],
        |body, expression, targets, origin| {
            targets.insert_type_descriptor(
                CompileTypeDescriptor {
                    id: ENUM_TYPE,
                    canonical_name: "constructors::Choice".to_owned(),
                    class: CompileTypeClass::ScriptEnum,
                    shape: None,
                    fields: Vec::new(),
                    variants: vec![VARIANT],
                },
                origin,
            )?;
            targets.insert_variant_descriptor(
                CompileVariantDescriptor {
                    id: VARIANT,
                    owner: ENUM_TYPE,
                    name: "Pair".to_owned(),
                    fields: vec![left, right],
                    declaration_order: 0,
                },
                origin,
            )?;
            for (index, field) in [left, right].into_iter().enumerate() {
                targets.insert_field_descriptor(
                    CompileFieldDescriptor {
                        id: field,
                        owner: ENUM_TYPE,
                        variant: Some(VARIANT),
                        name: format!("_{index}"),
                        contract: None,
                        declaration_order: u32::try_from(index).expect("field index"),
                        access: CompileFieldAccess::script(),
                        host_runtime: None,
                    },
                    origin,
                )?;
            }
            let values = call_values(body, expression);
            targets.insert_constructor(
                ROOT_FUNCTION,
                expression,
                CompileConstructorTarget::Variant {
                    type_id: ENUM_TYPE,
                    variant: VARIANT,
                    evaluation_order: values.clone(),
                    fields: vec![
                        CompileConstructorField {
                            field: left,
                            parameter: 0,
                            parameter_name: "_0".to_owned(),
                            value: CompileConstructorValue::Explicit {
                                source_index: 0,
                                value: values[0],
                            },
                        },
                        CompileConstructorField {
                            field: right,
                            parameter: 1,
                            parameter_name: "_1".to_owned(),
                            value: CompileConstructorValue::Explicit {
                                source_index: 1,
                                value: values[1],
                            },
                        },
                    ],
                },
                origin,
            )
        },
    )
    .expect("tuple variant constructor");

    let function = only_function(&program);
    let allocation = function
        .statements()
        .find(|(_, statement)| matches!(statement.kind, MirStatementKind::Allocate(_)))
        .expect("enum allocation")
        .1;
    assert!(matches!(
        &allocation.kind,
        MirStatementKind::Allocate(MirAggregate::Enum { type_id, variant, fields })
            if *type_id == ENUM_TYPE && *variant == VARIANT
                && fields.iter().map(|(field, _)| *field).collect::<Vec<_>>() == vec![left, right]
    ));
    let MirPlace::Temp(result) = allocation.destination.expect("enum result temp") else {
        panic!("enum constructor must define a temp")
    };
    assert_eq!(
        function.temp(result).expect("enum result").value_type,
        MirValueType::Enum(ENUM_TYPE)
    );
}

#[test]
fn constructor_builder_preserves_dynamic_names_and_source_order() {
    let record = lower(
        r#"fn main() { return Missing { second: "two", first: "one" }; }"#,
        r#"Missing { second: "two", first: "one" }"#,
        &[],
        |body, expression, targets, origin| {
            let fields = record_values(body, expression)
                .into_iter()
                .map(|(name, value)| CompileDynamicConstructorField { name, value })
                .collect();
            targets.insert_constructor(
                ROOT_FUNCTION,
                expression,
                CompileConstructorTarget::DynamicRecord {
                    type_name: "Missing".to_owned(),
                    fields,
                },
                origin,
            )
        },
    )
    .expect("dynamic record constructor");
    assert!(only_function(&record).statements().any(|(_, statement)| matches!(
        &statement.kind,
        MirStatementKind::Allocate(MirAggregate::DynamicRecord { type_name, fields })
            if type_name == "Missing"
                && fields.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>() == vec!["second", "first"]
    )));

    let variant = lower(
        r#"fn main() { return Missing::Ready { label: "ready", amount: 3 }; }"#,
        r#"Missing::Ready { label: "ready", amount: 3 }"#,
        &[],
        |body, expression, targets, origin| {
            let fields = record_values(body, expression)
                .into_iter()
                .map(|(name, value)| CompileDynamicConstructorField { name, value })
                .collect();
            targets.insert_constructor(
                ROOT_FUNCTION,
                expression,
                CompileConstructorTarget::DynamicVariant {
                    owner_name: "Missing".to_owned(),
                    variant_name: "Ready".to_owned(),
                    fields,
                },
                origin,
            )
        },
    )
    .expect("dynamic variant constructor");
    assert!(only_function(&variant).statements().any(|(_, statement)| matches!(
        &statement.kind,
        MirStatementKind::Allocate(MirAggregate::DynamicVariant { owner_name, variant_name, fields })
            if owner_name == "Missing" && variant_name == "Ready"
                && fields.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>() == vec!["label", "amount"]
    )));
}

#[test]
fn constructor_builder_stops_before_guard_or_allocation_after_terminating_field() {
    let field = FieldId::new(8_550);
    let source = "fn main() { return Missing { value: { return 9; 10 } }; }";
    let program = lower(
        source,
        "Missing { value: { return 9; 10 } }",
        &[],
        |body, expression, targets, origin| {
            insert_record_layout(targets, &[(field, "value", None)], origin)?;
            let value = record_values(body, expression)[0].1;
            targets.insert_constructor(
                ROOT_FUNCTION,
                expression,
                CompileConstructorTarget::Record {
                    type_id: RECORD_TYPE,
                    shape: RECORD_SHAPE,
                    evaluation_order: vec![value],
                    fields: vec![CompileConstructorField {
                        field,
                        parameter: 0,
                        parameter_name: "value".to_owned(),
                        value: CompileConstructorValue::Explicit {
                            source_index: 0,
                            value,
                        },
                    }],
                },
                origin,
            )
        },
    )
    .expect("terminating constructor field");

    let function = only_function(&program);
    assert!(!function.statements().any(|(_, statement)| matches!(
        statement.kind,
        MirStatementKind::Allocate(_) | MirStatementKind::GuardTrap { .. }
    )));
    assert!(program.dump().contains("-> return 9i64 [pure]"));
}
