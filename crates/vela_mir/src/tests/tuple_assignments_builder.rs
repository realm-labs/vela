use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::{ShapeId, SourceId};
use vela_def::{FieldId, FunctionId, TypeId};
use vela_hir::body::{HirExprKind, HirField};
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::ModulePath;

use crate::{
    CompileFieldAccess, CompileFieldDescriptor, CompileFieldTarget, CompileFunctionAccess,
    CompileFunctionClass, CompileFunctionDescriptor, CompileFunctionIdentity, CompileMemberTarget,
    CompileParameter, CompileParameterDefault, CompilePositionalPolicy, CompileSignature,
    CompileTargetSnapshot, CompileTypeClass, CompileTypeDescriptor, MirAggregate, MirBuildError,
    MirEffect, MirIndexOperation, MirLoweringConfig, MirLoweringInput, MirOperand, MirPlace,
    MirProgram, MirSourceOrigin, MirStatementKind,
};

const SOURCE: SourceId = SourceId::new(94);
const FUNCTION: FunctionId = FunctionId::new(9_400);
const HOLDER_TYPE: TypeId = TypeId::new(9_410);
const HOLDER_SHAPE: ShapeId = ShapeId::new(9_411);
const HOLDER_COORDS: FieldId = FieldId::new(9_412);
const CELL_TYPE: TypeId = TypeId::new(9_420);
const CELL_SHAPE: ShapeId = ShapeId::new(9_421);
const CELL_VALUE: FieldId = FieldId::new(9_422);

#[derive(Clone, Copy)]
enum NamedMemberPolicy {
    Stable,
    Dynamic,
}

#[derive(Clone, Copy)]
struct FixtureOptions {
    named: NamedMemberPolicy,
    tuple_index_override: Option<u32>,
}

impl FixtureOptions {
    const STABLE: Self = Self {
        named: NamedMemberPolicy::Stable,
        tuple_index_override: None,
    };

    const DYNAMIC: Self = Self {
        named: NamedMemberPolicy::Dynamic,
        tuple_index_override: None,
    };

    const fn tuple_index(index: u32) -> Self {
        Self {
            named: NamedMemberPolicy::Dynamic,
            tuple_index_override: Some(index),
        }
    }
}

fn lower_selected(source: &str, options: FixtureOptions) -> Result<MirProgram, MirBuildError> {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SOURCE,
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("tuple_assignments"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let body = graph.function_body(declaration.id).expect("main body");
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(FUNCTION, body.id)],
    )
    .expect("tuple assignment analysis");
    let origin = MirSourceOrigin::body(body.id, body.origin.span);
    let mut targets = CompileTargetSnapshot::builder();
    targets.insert_script_function(
        declaration.id,
        body.id,
        CompileFunctionDescriptor {
            id: FUNCTION,
            class: CompileFunctionClass::Script,
            canonical_symbol: "tuple_assignments::main".to_owned(),
            debug_name: "main".to_owned(),
            signature: CompileSignature {
                parameters: parameters(&graph, body),
                positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                return_contract: None,
                effect: MirEffect::PURE,
            },
            access: CompileFunctionAccess::script(false),
        },
        origin,
    )?;
    if matches!(options.named, NamedMemberPolicy::Stable) {
        insert_record_descriptors(&mut targets, origin)?;
    }
    for field in body.expressions.values().filter_map(|expression| {
        let HirExprKind::Field(field) = &expression.kind else {
            return None;
        };
        Some(field)
    }) {
        targets.insert_member(
            FUNCTION,
            field.expression,
            member_target(field, options),
            expression_origin(body, field.expression),
        )?;
    }
    let targets = targets.build()?;
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(FUNCTION),
        body.id,
        analysis.view(FUNCTION).expect("tuple analysis view"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )?;
    crate::build_mir(input)
}

fn parameters(graph: &ModuleGraph, body: &vela_hir::body::HirBody) -> Vec<CompileParameter> {
    let bindings = graph
        .bindings_for_body(body.id)
        .expect("tuple assignment bindings");
    body.params
        .iter()
        .map(|parameter| CompileParameter {
            name: bindings
                .local(parameter.local)
                .expect("parameter binding")
                .name
                .clone(),
            contract: None,
            default: CompileParameterDefault::Required,
            origin: None,
        })
        .collect()
}

fn member_target(field: &HirField, options: FixtureOptions) -> CompileMemberTarget {
    if let Ok(index) = field.name.parse::<u32>() {
        return CompileMemberTarget::TupleIndex(options.tuple_index_override.unwrap_or(index));
    }
    match options.named {
        NamedMemberPolicy::Dynamic => CompileMemberTarget::Dynamic {
            name: field.name.clone(),
        },
        NamedMemberPolicy::Stable => match field.name.as_str() {
            "coords" => CompileMemberTarget::ScriptField(CompileFieldTarget::RecordSlot {
                type_id: HOLDER_TYPE,
                shape: HOLDER_SHAPE,
                field: HOLDER_COORDS,
            }),
            "value" => CompileMemberTarget::ScriptField(CompileFieldTarget::RecordSlot {
                type_id: CELL_TYPE,
                shape: CELL_SHAPE,
                field: CELL_VALUE,
            }),
            name => panic!("unexpected stable tuple fixture field {name:?}"),
        },
    }
}

fn insert_record_descriptors(
    targets: &mut crate::CompileTargetSnapshotBuilder,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    for (type_id, shape, name, field, field_name) in [
        (
            HOLDER_TYPE,
            HOLDER_SHAPE,
            "tuple_assignments::Holder",
            HOLDER_COORDS,
            "coords",
        ),
        (
            CELL_TYPE,
            CELL_SHAPE,
            "tuple_assignments::Cell",
            CELL_VALUE,
            "value",
        ),
    ] {
        targets.insert_type_descriptor(
            CompileTypeDescriptor {
                id: type_id,
                canonical_name: name.to_owned(),
                runtime_name: name.to_owned(),
                class: CompileTypeClass::ScriptRecord,
                shape: Some(shape),
                fields: vec![field],
                variants: Vec::new(),
            },
            origin,
        )?;
        targets.insert_field_descriptor(
            CompileFieldDescriptor {
                id: field,
                owner: type_id,
                variant: None,
                name: field_name.to_owned(),
                contract: None,
                declaration_order: 0,
                access: CompileFieldAccess::script(),
            },
            origin,
        )?;
    }
    Ok(())
}

fn expression_origin(
    body: &vela_hir::body::HirBody,
    expression: vela_hir::ids::HirExprId,
) -> MirSourceOrigin {
    let expression = body.expression(expression).expect("fixture expression");
    MirSourceOrigin::expression(body.id, expression.id, expression.origin.span)
}

fn only_function(program: &MirProgram) -> &crate::MirFunction {
    let functions = program.functions().collect::<Vec<_>>();
    let [(_, function)] = functions.as_slice() else {
        panic!("expected one MIR function, got {}", functions.len())
    };
    function
}

fn statement_positions(
    function: &crate::MirFunction,
    predicate: impl Fn(&MirStatementKind) -> bool,
) -> Vec<usize> {
    function
        .statements()
        .enumerate()
        .filter_map(|(index, (_, statement))| predicate(&statement.kind).then_some(index))
        .collect()
}

#[test]
fn tuple_assignment_rebuilds_a_direct_local_and_returns_the_leaf() {
    let program = lower_selected(
        include_str!("../../tests/fixtures/builder/tuple_writeback.vela"),
        FixtureOptions::DYNAMIC,
    )
    .expect("direct tuple assignment");
    assert_eq!(
        program.dump(),
        include_str!("../../tests/fixtures/builder/tuple_writeback.expected")
    );
    let function = only_function(&program);
    let pair = function.parameters()[0].storage;
    let rhs = function.parameters()[1].storage;
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();

    assert!(matches!(
        statements[0].kind,
        MirStatementKind::Assign(crate::MirRvalue::Use(MirOperand::Local(local))) if local == pair
    ));
    assert!(matches!(
        statements[1].kind,
        MirStatementKind::Assign(crate::MirRvalue::Use(MirOperand::Local(local))) if local == rhs
    ));
    assert!(matches!(
        statements[2].kind,
        MirStatementKind::TupleField { index: 0, .. }
    ));
    let (rebuilt, safepoint) = match (&statements[3].destination, &statements[3].kind) {
        (
            Some(MirPlace::Temp(rebuilt)),
            MirStatementKind::Allocate(MirAggregate::Tuple(values)),
        ) => {
            assert_eq!(values.len(), 2);
            assert!(matches!(values[0], MirOperand::Temp(_)));
            assert!(matches!(values[1], MirOperand::Temp(_)));
            (*rebuilt, statements[3].safepoint)
        }
        other => panic!("expected rebuilt tuple allocation, got {other:?}"),
    };
    assert!(safepoint.is_some());
    assert!(statements[3].effect.may_allocate);
    assert!(matches!(
        statements[4].kind,
        MirStatementKind::Assign(crate::MirRvalue::Use(MirOperand::Temp(value))) if value == rebuilt
    ));
    assert_eq!(statements[4].destination, Some(MirPlace::local(pair)));
    assert!(matches!(
        function
            .blocks()
            .find_map(|(_, block)| block.terminator())
            .expect("return terminator")
            .kind,
        crate::MirTerminatorKind::Return(Some(MirOperand::Temp(_)))
    ));
}

#[test]
fn tuple_compound_reads_the_leaf_after_rhs_then_rebuilds() {
    let program = lower_selected(
        "fn main(pair: (i64, i64), rhs: i64) { return pair.1 += rhs; }",
        FixtureOptions::DYNAMIC,
    )
    .expect("compound tuple assignment");
    let function = only_function(&program);
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let rhs_capture = statements
        .iter()
        .position(|statement| {
            matches!(
                statement.kind,
                MirStatementKind::Assign(crate::MirRvalue::Use(MirOperand::Local(local)))
                    if local == function.parameters()[1].storage
            )
        })
        .expect("RHS capture");
    let reads = statement_positions(function, |kind| {
        matches!(kind, MirStatementKind::TupleField { .. })
    });
    let compound = statements
        .iter()
        .position(|statement| matches!(statement.kind, MirStatementKind::Binary { .. }))
        .expect("typed compound op");
    let allocation = statements
        .iter()
        .position(|statement| matches!(statement.kind, MirStatementKind::Allocate(_)))
        .expect("tuple rebuild");
    assert!(rhs_capture < reads[0]);
    assert!(reads[0] < compound);
    assert!(compound < reads[1]);
    assert!(reads[1] < allocation);
}

#[test]
fn nested_tuple_assignment_rebuilds_inner_before_outer() {
    let program = lower_selected(
        "fn main(outer: ((i64, i64), i64), rhs: i64) { return (outer.0).1 = rhs; }",
        FixtureOptions::DYNAMIC,
    )
    .expect("nested tuple assignment");
    let function = only_function(&program);
    let allocations = function
        .statements()
        .filter_map(|(_, statement)| match &statement.kind {
            MirStatementKind::Allocate(MirAggregate::Tuple(values)) => {
                Some((values.len(), statement.origin))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(allocations.len(), 2, "{}", program.dump());
    assert_eq!(allocations[0].0, 2);
    assert_eq!(allocations[1].0, 2);
    assert_ne!(allocations[0].1, allocations[1].1);
    assert_eq!(
        function
            .statements()
            .filter(|(_, statement)| matches!(statement.kind, MirStatementKind::TupleField { .. }))
            .count(),
        3
    );
}

#[test]
fn tuples_rebuild_below_and_above_stable_or_dynamic_record_fields() {
    let below_source = r#"
struct Holder { coords: (i64, i64) }
fn main(record: Holder, rhs: i64) { return record.coords.1 = rhs; }
"#;
    for options in [FixtureOptions::STABLE, FixtureOptions::DYNAMIC] {
        let program = lower_selected(below_source, options).expect("tuple below record field");
        let function = only_function(&program);
        let allocation = statement_positions(function, |kind| {
            matches!(kind, MirStatementKind::Allocate(MirAggregate::Tuple(_)))
        });
        let write = statement_positions(function, |kind| {
            matches!(kind, MirStatementKind::WriteField { .. })
        });
        assert_eq!(allocation.len(), 1, "{}", program.dump());
        assert_eq!(write.len(), 1, "{}", program.dump());
        assert!(allocation[0] < write[0], "{}", program.dump());
    }

    let above_source = r#"
struct Cell { value: i64 }
fn main(pair: (Cell, i64), rhs: i64) { return pair.0.value = rhs; }
"#;
    for options in [FixtureOptions::STABLE, FixtureOptions::DYNAMIC] {
        let program = lower_selected(above_source, options).expect("tuple above record field");
        let function = only_function(&program);
        let write = statement_positions(function, |kind| {
            matches!(kind, MirStatementKind::WriteField { .. })
        });
        let allocation = statement_positions(function, |kind| {
            matches!(kind, MirStatementKind::Allocate(MirAggregate::Tuple(_)))
        });
        assert_eq!(write.len(), 1, "{}", program.dump());
        assert_eq!(allocation.len(), 1, "{}", program.dump());
        assert!(write[0] < allocation[0], "{}", program.dump());
    }
}

#[test]
fn indexed_mixed_tuple_record_chain_writes_rebuilt_root_through_same_index() {
    let source = r#"
struct Cell { value: i64 }
fn main(items: Array<((Cell, i64), i64)>, index: i64, rhs: i64) {
    return ((items[index].0).0).value = rhs;
}
"#;
    let program = lower_selected(source, FixtureOptions::STABLE)
        .expect("indexed mixed tuple/record assignment");
    let function = only_function(&program);
    let index_reads = statement_positions(function, |kind| {
        matches!(
            kind,
            MirStatementKind::Index(MirIndexOperation::Read { .. })
        )
    });
    let index_writes = statement_positions(function, |kind| {
        matches!(
            kind,
            MirStatementKind::Index(MirIndexOperation::Write { .. })
        )
    });
    let tuple_allocations = statement_positions(function, |kind| {
        matches!(kind, MirStatementKind::Allocate(MirAggregate::Tuple(_)))
    });
    assert_eq!(index_reads.len(), 1, "{}", program.dump());
    assert_eq!(index_writes.len(), 1, "{}", program.dump());
    assert_eq!(tuple_allocations.len(), 2, "{}", program.dump());
    assert!(index_reads[0] < tuple_allocations[0]);
    assert!(tuple_allocations[1] < index_writes[0]);
    let read = function
        .statements()
        .nth(index_reads[0])
        .map(|(_, statement)| statement)
        .expect("indexed root read statement");
    let write = function
        .statements()
        .nth(index_writes[0])
        .map(|(_, statement)| statement)
        .expect("indexed write statement");
    let (
        MirStatementKind::Index(MirIndexOperation::Read {
            receiver: read_receiver,
            index: read_index,
        }),
        MirStatementKind::Index(MirIndexOperation::Write {
            receiver: write_receiver,
            index: write_index,
            ..
        }),
    ) = (&read.kind, &write.kind)
    else {
        unreachable!("indexed statement positions were classified above")
    };
    assert_eq!(read_receiver, write_receiver);
    assert_eq!(read_index, write_index);
    assert!(write.effect.may_allocate);
    assert!(write.safepoint.is_some());
}

#[test]
fn mixed_tuple_compound_observes_aliasing_rhs_write() {
    let source = r#"
struct Cell { value: i64 }
fn main(pair: (Cell, i64), rhs: i64) {
    return (pair.0).value += ((pair.0).value = rhs);
}
"#;
    let program =
        lower_selected(source, FixtureOptions::STABLE).expect("aliasing mixed assignment");
    let function = only_function(&program);
    let statements = function
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let rhs_write = statements
        .iter()
        .position(|statement| matches!(statement.kind, MirStatementKind::WriteField { .. }))
        .expect("RHS alias write");
    let current_read = statements
        .iter()
        .enumerate()
        .skip(rhs_write + 1)
        .find_map(|(index, statement)| {
            matches!(statement.kind, MirStatementKind::ReadField { .. }).then_some(index)
        })
        .expect("post-RHS compound leaf read");
    let compound = statements
        .iter()
        .enumerate()
        .skip(current_read + 1)
        .find_map(|(index, statement)| {
            matches!(
                statement.kind,
                MirStatementKind::Binary { .. } | MirStatementKind::DynamicBinary { .. }
            )
            .then_some(index)
        })
        .unwrap_or_else(|| panic!("compound operation:\n{}", program.dump()));
    assert!(rhs_write < current_read);
    assert!(current_read < compound);
}

#[test]
fn tuple_assignment_stops_when_an_index_component_diverges() {
    let source = r#"
fn main(items: Array<(i64, i64)>, index: i64, rhs: i64) {
    items[{ return index; }].1 = rhs;
    return 0;
}
"#;
    let program =
        lower_selected(source, FixtureOptions::DYNAMIC).expect("diverging indexed tuple target");
    let function = only_function(&program);
    assert!(!function.statements().any(|(_, statement)| matches!(
        statement.kind,
        MirStatementKind::Allocate(MirAggregate::Tuple(_))
            | MirStatementKind::Index(MirIndexOperation::Write { .. })
    )));
    assert!(!program.dump().contains("-> return 0i64"));
}

#[test]
fn tuple_assignment_rejects_unknown_non_tuple_out_of_range_and_mismatched_facts() {
    let unknown = lower_selected(
        "fn main(pair, rhs) { return pair.1 = rhs; }",
        FixtureOptions::DYNAMIC,
    )
    .expect_err("unknown tuple arity must fail");
    assert_error_contains(unknown, "requires an exact tuple analysis fact");

    let non_tuple = lower_selected(
        "fn main(pair: i64, rhs: i64) { return pair.1 = rhs; }",
        FixtureOptions::DYNAMIC,
    )
    .expect_err("known non-tuple receiver must fail");
    assert_error_contains(non_tuple, "requires an exact tuple analysis fact");

    let out_of_range = lower_selected(
        "fn main(pair: (i64, i64), rhs: i64) { return pair.5 = rhs; }",
        FixtureOptions::DYNAMIC,
    )
    .expect_err("out-of-range tuple target must fail");
    assert_error_contains(out_of_range, "out of range for arity 2");

    let mismatch = lower_selected(
        "fn main(pair: (i64, i64), rhs: i64) { return pair.1 = rhs; }",
        FixtureOptions::tuple_index(0),
    )
    .expect_err("compile target/HIR tuple mismatch must fail");
    assert_error_contains(mismatch, "disagrees with HIR member 1");
}

fn assert_error_contains(error: MirBuildError, expected: &str) {
    let MirBuildError::InconsistentInput { message, origin } = error else {
        panic!("expected inconsistent tuple input, got {error:?}")
    };
    assert!(message.contains(expected), "{message:?}");
    assert_eq!(origin.span.source, SOURCE);
}
