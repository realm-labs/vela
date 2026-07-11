use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::{HostTypeId, PrimitiveTag, ShapeId, SourceId};
use vela_def::{FieldId, FunctionId, TypeId, VariantId};
use vela_hir::body::{HirExprKind, HirField};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use crate::{
    CompileFieldAccess, CompileFieldDescriptor, CompileFieldTarget, CompileFunctionAccess,
    CompileFunctionClass, CompileFunctionDescriptor, CompileFunctionIdentity,
    CompileHostIndexCapability, CompileHostPathSegment, CompileHostPathTarget, CompileMemberTarget,
    CompileParameter, CompileParameterDefault, CompilePositionalPolicy, CompileSignature,
    CompileTargetSnapshot, CompileTypeClass, CompileTypeDescriptor, CompileVariantDescriptor,
    HostFieldTarget, HostTypeTarget, MirAggregate, MirBuildError, MirEffect, MirHostOperation,
    MirIndexOperation, MirLoweringConfig, MirLoweringInput, MirOperand, MirPlace, MirProgram,
    MirSourceOrigin, MirStatementKind, MirTypeContract,
};

const SOURCE: SourceId = SourceId::new(94);
const FUNCTION: FunctionId = FunctionId::new(9_400);
const HOLDER_TYPE: TypeId = TypeId::new(9_410);
const HOLDER_SHAPE: ShapeId = ShapeId::new(9_411);
const HOLDER_COORDS: FieldId = FieldId::new(9_412);
const CELL_TYPE: TypeId = TypeId::new(9_420);
const CELL_SHAPE: ShapeId = ShapeId::new(9_421);
const CELL_VALUE: FieldId = FieldId::new(9_422);
const HOST_TYPE_ID: TypeId = TypeId::new(9_430);
const HOST_RUNTIME_ID: HostTypeId = HostTypeId::new(9_431);
const HOST_COORDS: FieldId = FieldId::new(9_432);
const HOST_TUPLES: FieldId = FieldId::new(9_433);
const HOST_VARIANT: VariantId = VariantId::new(9_434);
const HOST_TYPE: HostTypeTarget = HostTypeTarget {
    semantic: HOST_TYPE_ID,
    runtime: HOST_RUNTIME_ID,
};

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
                host_runtime: None,
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

#[derive(Clone)]
enum HostPrefixKind {
    Field {
        variant: bool,
    },
    Index {
        capability: CompileHostIndexCapability,
    },
}

fn lower_host_tuple(
    prefix: HostPrefixKind,
    access: CompileFieldAccess,
) -> Result<MirProgram, MirBuildError> {
    let variant = matches!(&prefix, HostPrefixKind::Field { variant: true });
    let (source, field_name, field_id, field_contract) = match &prefix {
        HostPrefixKind::Field { .. } => (
            "struct Root { coords: (i64, i64) } fn main(host: Root, rhs: i64) { return host.coords.1 = rhs; }",
            "coords",
            HOST_COORDS,
            MirTypeContract::Tuple(vec![
                Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
                Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
            ]),
        ),
        HostPrefixKind::Index { .. } => (
            "struct Root { tuples: Array<(i64, i64)> } fn main(host: Root, index: i64, rhs: i64) { return host.tuples[index].1 = rhs; }",
            "tuples",
            HOST_TUPLES,
            MirTypeContract::Array(Some(Box::new(MirTypeContract::Tuple(vec![
                Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
                Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
            ])))),
        ),
    };
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SOURCE,
        ModulePath::from_qualified("tuple_host_assignments"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("host tuple main");
    let body = graph
        .function_body(declaration.id)
        .expect("host tuple body");
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(FUNCTION, body.id)],
    )
    .expect("host tuple analysis");
    let origin = MirSourceOrigin::body(body.id, body.origin.span);
    let host_field = body
        .expressions
        .values()
        .find_map(|expression| {
            let HirExprKind::Field(field) = &expression.kind else {
                return None;
            };
            (field.name == field_name).then_some(field)
        })
        .expect("host prefix field");
    let tuple_field = body
        .expressions
        .values()
        .find_map(|expression| {
            let HirExprKind::Field(field) = &expression.kind else {
                return None;
            };
            (field.name == "1").then_some(field)
        })
        .expect("tuple suffix field");
    let host_target = HostFieldTarget {
        owner: HOST_TYPE,
        semantic: field_id,
        runtime: field_id,
        access: access.clone(),
    };
    let mut targets = CompileTargetSnapshot::builder();
    targets.insert_script_function(
        declaration.id,
        body.id,
        CompileFunctionDescriptor {
            id: FUNCTION,
            class: CompileFunctionClass::Script,
            canonical_symbol: "tuple_host_assignments::main".to_owned(),
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
    targets.insert_type_descriptor(
        CompileTypeDescriptor {
            id: HOST_TYPE_ID,
            canonical_name: "host::TupleRoot".to_owned(),
            runtime_name: "TupleRoot".to_owned(),
            class: CompileTypeClass::Host {
                runtime: HOST_RUNTIME_ID,
            },
            shape: None,
            fields: if variant { Vec::new() } else { vec![field_id] },
            variants: if variant {
                vec![HOST_VARIANT]
            } else {
                Vec::new()
            },
        },
        origin,
    )?;
    if variant {
        targets.insert_variant_descriptor(
            CompileVariantDescriptor {
                id: HOST_VARIANT,
                owner: HOST_TYPE_ID,
                name: "TupleState".to_owned(),
                fields: vec![field_id],
                declaration_order: 0,
            },
            origin,
        )?;
    }
    targets.insert_field_descriptor(
        CompileFieldDescriptor {
            id: field_id,
            owner: HOST_TYPE_ID,
            variant: variant.then_some(HOST_VARIANT),
            name: field_name.to_owned(),
            contract: Some(field_contract),
            declaration_order: 0,
            access: access.clone(),
            host_runtime: Some(field_id),
        },
        origin,
    )?;
    targets.insert_host_path(
        FUNCTION,
        host_field.receiver,
        CompileHostPathTarget {
            root: host_field.receiver,
            root_type: HOST_TYPE,
            segments: Vec::new(),
        },
        expression_origin(body, host_field.receiver),
    )?;
    targets.insert_member(
        FUNCTION,
        host_field.expression,
        CompileMemberTarget::HostField(host_target.clone()),
        expression_origin(body, host_field.expression),
    )?;
    let field_segment = match &prefix {
        HostPrefixKind::Field { variant: true } => {
            CompileHostPathSegment::VariantField(host_target.clone())
        }
        HostPrefixKind::Field { variant: false } | HostPrefixKind::Index { .. } => {
            CompileHostPathSegment::Field(host_target.clone())
        }
    };
    let mut segments = vec![field_segment];
    let prefix_expression = match prefix {
        HostPrefixKind::Field { .. } => host_field.expression,
        HostPrefixKind::Index { capability } => {
            let index = body
                .expressions
                .values()
                .find_map(|expression| {
                    let HirExprKind::Index(index) = &expression.kind else {
                        return None;
                    };
                    (index.receiver == host_field.expression).then_some(index)
                })
                .expect("host tuple index prefix");
            segments.push(CompileHostPathSegment::DynamicIndex {
                expression: index.index,
                capability,
            });
            index.expression
        }
    };
    assert_eq!(tuple_field.receiver, prefix_expression);
    targets.insert_host_path(
        FUNCTION,
        host_field.expression,
        CompileHostPathTarget {
            root: host_field.receiver,
            root_type: HOST_TYPE,
            segments: vec![segments[0].clone()],
        },
        expression_origin(body, host_field.expression),
    )?;
    if prefix_expression != host_field.expression {
        targets.insert_host_path(
            FUNCTION,
            prefix_expression,
            CompileHostPathTarget {
                root: host_field.receiver,
                root_type: HOST_TYPE,
                segments,
            },
            expression_origin(body, prefix_expression),
        )?;
    }
    targets.insert_member(
        FUNCTION,
        tuple_field.expression,
        CompileMemberTarget::TupleIndex(1),
        expression_origin(body, tuple_field.expression),
    )?;
    let targets = targets.build()?;
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(FUNCTION),
        body.id,
        analysis.view(FUNCTION).expect("host tuple analysis view"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )?;
    crate::build_mir(input)
}

fn host_access(readable: bool, writable: bool) -> CompileFieldAccess {
    CompileFieldAccess::new(readable, writable, true, true, Vec::new())
}

fn tuple_index_capability(readable: bool, writable: bool) -> CompileHostIndexCapability {
    CompileHostIndexCapability {
        readable,
        writable,
        mutable: writable,
        removable: writable,
        key: Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
        value: Some(MirTypeContract::Tuple(vec![
            Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
            Some(MirTypeContract::Primitive(PrimitiveTag::I64)),
        ])),
    }
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
        "fn main(pair: (i64, i64), rhs: i64) { return pair.1 = rhs; }",
        FixtureOptions::DYNAMIC,
    )
    .expect("direct tuple assignment");
    assert_eq!(
        program.dump(),
        r#"mir {
  target function#9400 CompileFunctionDescriptor { id: FunctionId(9400), class: Script, canonical_symbol: "tuple_assignments::main", debug_name: "main", signature: CompileSignature { parameters: [CompileParameter { name: "pair", contract: None, default: Required, origin: None }, CompileParameter { name: "rhs", contract: None, default: Required, origin: None }], positional: ExactOrTrailingDefaults, return_contract: None, effect: MirEffect { may_trap: false, may_allocate: false, script_call: false, dynamic_call: false, global_read: false, host_read: false, host_write: false, host_call: false, reflection_read: false, reflection_write: false, reflection_call: false, emits_event: false, reads_time: false, uses_random: false, reads_io: false, writes_io: false } }, access: CompileFunctionAccess { public: false, reflect_visible: true, reflect_callable: false } }
  fn f0 body h0 owner function#9400 symbol="tuple_assignments::main" @94:36..60/h0 {
    param p0: pair -> l0 kind=Explicit(HirParamId(0)) contract=None default=None hir=l0 @94:8..12/h0
    param p1: rhs -> l1 kind=Explicit(HirParamId(1)) contract=None default=None hir=l1 @94:26..29/h0
    local l0: Script(HirLocalId(0)) Tuple(2) @94:8..12/h0
    local l1: Script(HirLocalId(1)) Primitive(I64) @94:26..29/h0
    temp t0: Tuple(2) def=s0 @94:45..49/e2
    temp t1: Primitive(I64) def=s1 @94:54..57/e3
    temp t2: Primitive(I64) def=s2 @94:45..51/e1
    temp t3: Tuple(2) def=s3 @94:45..51/e1
    debug dl0: pair -> l0 kind=Parameter hir=Some(0) scope=h0 live=[] @94:8..12/h0
    debug dl1: rhs -> l1 kind=Parameter hir=Some(1) scope=h0 live=[] @94:26..29/h0
    safepoint sp0: live={} @94:45..51/e1
    bb0:
      s0: t0 = l0 [pure] @94:45..49/e2
      s1: t1 = l1 [pure] @94:54..57/e3
      s2: t2 = tuple.field t0.0 [trap] @94:45..51/e1
      s3: t3 = alloc.tuple[t2, t1] [trap|alloc, sp0] @94:45..51/e1
      s4: l0 = t3 [pure] @94:45..57/e0
      -> return t1 [pure] @94:38..58/s0
  }
}
"#
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
fn host_tuple_assignment_reads_rebuilds_and_writes_the_exact_prefix() {
    for prefix in [
        HostPrefixKind::Field { variant: false },
        HostPrefixKind::Index {
            capability: tuple_index_capability(true, true),
        },
    ] {
        let program = lower_host_tuple(prefix, host_access(true, true))
            .expect("writable HostAccess tuple prefix");
        let function = only_function(&program);
        let host = function
            .statements()
            .filter_map(|(_, statement)| match &statement.kind {
                MirStatementKind::Host(operation) => Some((statement, operation)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [
            (
                read_statement,
                MirHostOperation::Read {
                    root: read_root,
                    path: read_path,
                },
            ),
            (
                write_statement,
                MirHostOperation::Write {
                    root: write_root,
                    path: write_path,
                    value,
                },
            ),
        ] = host.as_slice()
        else {
            panic!(
                "expected one HostAccess read/write pair:\n{}",
                program.dump()
            )
        };
        assert_eq!(read_root, write_root);
        assert_eq!(read_path, write_path);
        assert!(matches!(value, MirOperand::Temp(_)));
        assert!(read_statement.effect.host_read && read_statement.effect.may_allocate);
        assert!(read_statement.safepoint.is_some());
        assert!(write_statement.effect.host_read && write_statement.effect.host_write);
        assert!(write_statement.safepoint.is_none());
        let read = function
            .statements()
            .position(|(_, statement)| std::ptr::eq(statement, *read_statement))
            .expect("host read order");
        let allocate = statement_positions(function, |kind| {
            matches!(kind, MirStatementKind::Allocate(MirAggregate::Tuple(_)))
        })[0];
        let write = function
            .statements()
            .position(|(_, statement)| std::ptr::eq(statement, *write_statement))
            .expect("host write order");
        assert!(read < allocate && allocate < write);
        assert!(!function.statements().any(|(_, statement)| matches!(
            statement.kind,
            MirStatementKind::WriteField { .. }
                | MirStatementKind::Index(MirIndexOperation::Write { .. })
        )));
    }
}

#[test]
fn host_tuple_assignment_enforces_composite_read_and_write_access() {
    let unreadable = lower_host_tuple(
        HostPrefixKind::Field { variant: false },
        host_access(false, true),
    )
    .expect_err("tuple writeback must read its host prefix");
    assert_error_contains(unreadable, "prefix is not readable");

    let readonly = lower_host_tuple(
        HostPrefixKind::Field { variant: false },
        host_access(true, false),
    )
    .expect_err("ordinary host field must be writable");
    assert_error_contains(readonly, "prefix is not writable");

    lower_host_tuple(
        HostPrefixKind::Field { variant: true },
        host_access(true, false),
    )
    .expect("variant-field write policy authorizes tuple writeback");

    let unreadable_index = lower_host_tuple(
        HostPrefixKind::Index {
            capability: tuple_index_capability(false, true),
        },
        host_access(true, true),
    )
    .expect_err("host index prefix must be readable");
    assert_error_contains(unreadable_index, "prefix is not readable");

    let readonly_index = lower_host_tuple(
        HostPrefixKind::Index {
            capability: tuple_index_capability(true, false),
        },
        host_access(true, true),
    )
    .expect_err("host index prefix must be writable");
    assert_error_contains(readonly_index, "prefix is not writable");
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
