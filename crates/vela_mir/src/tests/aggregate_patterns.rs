use vela_common::{PrimitiveTag, ShapeId, SourceId, Span};
use vela_def::{FunctionId, TypeId, VariantId};
use vela_hir::ids::HirBodyId;

use crate::{
    MirAggregate, MirEffect, MirFunction, MirFunctionOwner, MirImmediate, MirOperand,
    MirPatternPredicate, MirPlace, MirProgram, MirRvalue, MirSafepoint, MirSourceOrigin,
    MirStatement, MirStatementKind, MirTargetTable, MirValueType,
};

fn origin(body: HirBodyId) -> MirSourceOrigin {
    MirSourceOrigin::body(body, Span::new(SourceId::new(41), 10, 30))
}

fn function(body: HirBodyId, origin: MirSourceOrigin) -> MirFunction {
    MirFunction::new(
        body,
        MirFunctionOwner::Function(FunctionId::new(700)),
        "test::aggregate_patterns",
        None,
        origin,
    )
}

#[test]
fn mir_model_keeps_dynamic_aggregate_field_evaluation_order_explicit() {
    let body = HirBodyId::new(701);
    let origin = origin(body);
    let mut function = function(body, origin);
    let entry = function.entry_block();
    let safepoint = function.add_safepoint(MirSafepoint::new(origin));

    for aggregate in [
        MirAggregate::DynamicRecord {
            type_name: "ExternalRecord".to_owned(),
            fields: vec![
                (
                    "second".to_owned(),
                    MirOperand::Immediate(MirImmediate::Unit),
                ),
                (
                    "first".to_owned(),
                    MirOperand::Immediate(MirImmediate::Bool(true)),
                ),
            ],
        },
        MirAggregate::DynamicVariant {
            owner_name: "ExternalState".to_owned(),
            variant_name: "Ready".to_owned(),
            fields: vec![
                (
                    "1".to_owned(),
                    MirOperand::Immediate(MirImmediate::Bool(false)),
                ),
                ("0".to_owned(), MirOperand::Immediate(MirImmediate::Unit)),
            ],
        },
    ] {
        let destination = function.add_temp(MirValueType::Dynamic, origin);
        function
            .append_statement(
                entry,
                MirStatement::new(
                    origin,
                    Some(MirPlace::temp(destination)),
                    MirStatementKind::Allocate(aggregate),
                    MirEffect::allocation(),
                    Some(safepoint),
                ),
            )
            .expect("dynamic aggregate allocation should satisfy model invariants");
    }

    let mut program = MirProgram::new(MirTargetTable::default());
    program
        .add_function(function)
        .expect("fixture function identity should be unique");
    let dump = program.dump();

    assert!(dump.contains(
        "alloc.record.dynamic type=\"ExternalRecord\" [\"second\"=unit, \"first\"=true]"
    ));
    assert!(dump.contains(
        "alloc.variant.dynamic owner=\"ExternalState\" variant=\"Ready\" [\"1\"=false, \"0\"=unit]"
    ));
}

#[test]
fn mir_model_pattern_predicates_are_pure_and_projection_complete() {
    let body = HirBodyId::new(702);
    let origin = origin(body);
    let mut function = function(body, origin);
    let entry = function.entry_block();
    let scrutinee = function.add_synthetic_local(MirValueType::Dynamic, origin);
    let value = MirOperand::Local(scrutinee);
    let type_id = TypeId::new(703);
    let shape = ShapeId::new(704);
    let variant = VariantId::new(705);

    let predicates = [
        MirPatternPredicate::TupleArity {
            value: value.clone(),
            arity: 2,
        },
        MirPatternPredicate::RecordShape {
            value: value.clone(),
            type_id,
            shape,
        },
        MirPatternPredicate::VariantShape {
            value: value.clone(),
            type_id,
            variant,
        },
        MirPatternPredicate::DynamicRecord {
            value: value.clone(),
            type_name: "ExternalRecord".to_owned(),
            required_fields: vec!["name".to_owned(), "level".to_owned()],
        },
        MirPatternPredicate::DynamicVariant {
            value,
            owner_name: "ExternalState".to_owned(),
            variant_name: "Ready".to_owned(),
            required_fields: vec!["payload".to_owned()],
        },
    ];

    for predicate in predicates {
        let destination = function.add_temp(MirValueType::Primitive(PrimitiveTag::Bool), origin);
        let statement = MirStatement::assign(
            origin,
            MirPlace::temp(destination),
            MirRvalue::PatternPredicate(predicate),
        );
        assert_eq!(statement.effect, MirEffect::PURE);
        assert_eq!(statement.safepoint, None);
        function
            .append_statement(entry, statement)
            .expect("pattern predicates should be accepted as non-trapping pure rvalues");
    }

    let mut program = MirProgram::new(MirTargetTable::default());
    program
        .add_function(function)
        .expect("fixture function identity should be unique");
    let dump = program.dump();

    for expected in [
        "pattern.tuple-arity l0 == 2 [pure]",
        "pattern.record-shape l0 type#703 shape#704 [pure]",
        "pattern.variant-shape l0 type#703 variant#705 [pure]",
        "pattern.record.dynamic l0 type=\"ExternalRecord\" fields=[\"name\", \"level\"] [pure]",
        "pattern.variant.dynamic l0 owner=\"ExternalState\" variant=\"Ready\" fields=[\"payload\"] [pure]",
    ] {
        assert!(dump.contains(expected), "missing `{expected}` in:\n{dump}");
    }
}
