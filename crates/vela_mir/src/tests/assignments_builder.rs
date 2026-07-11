use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::{ScalarValue, ShapeId, SourceId};
use vela_def::{FieldId, FunctionId, MethodId, TypeId};
use vela_hir::body::{HirExprKind, HirField};
use vela_hir::ids::{HirExprId, HirNodeId};
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use crate::{
    CompileFieldAccess, CompileFieldDescriptor, CompileFieldTarget, CompileFunctionAccess,
    CompileFunctionClass, CompileFunctionDescriptor, CompileFunctionIdentity, CompileMemberTarget,
    CompileMethodAccess, CompileMethodClass, CompileMethodDescriptor, CompileParameter,
    CompileParameterDefault, CompilePositionalPolicy, CompileSignature, CompileTargetSnapshot,
    CompileTypeClass, CompileTypeDescriptor, MethodExecutableTarget, MirEffect, MirIndexKey,
    MirIndexOperation, MirLoweringConfig, MirLoweringInput, MirOperand, MirProgram,
    MirSourceOrigin, MirStatementKind,
};

const ROOT_FUNCTION: FunctionId = FunctionId::new(8_200);
const PLAYER_TYPE: TypeId = TypeId::new(8_201);
const PLAYER_SHAPE: ShapeId = ShapeId::new(8_202);
const PLAYER_OUTER: FieldId = FieldId::new(8_203);
const STATS_TYPE: TypeId = TypeId::new(8_204);
const STATS_SHAPE: ShapeId = ShapeId::new(8_205);
const STATS_INNER: FieldId = FieldId::new(8_206);
const METHOD_OWNER: TypeId = TypeId::new(8_207);
const METHOD: MethodId = MethodId::new(8_208);
const METHOD_FUNCTION: FunctionId = FunctionId::new(8_209);
const METHOD_TARGET: MethodExecutableTarget = MethodExecutableTarget {
    method: METHOD,
    function: METHOD_FUNCTION,
    owner: METHOD_OWNER,
    node: HirNodeId::new(8_210),
};

#[derive(Clone, Copy)]
enum MemberPolicy {
    Dynamic,
    Tuple,
    StableNested,
    ScriptMethod,
    ValueMethod,
}

#[derive(Clone, Copy)]
struct FixtureOptions {
    members: MemberPolicy,
}

impl FixtureOptions {
    const DYNAMIC: Self = Self {
        members: MemberPolicy::Dynamic,
    };

    const TUPLE: Self = Self {
        members: MemberPolicy::Tuple,
    };

    const STABLE_NESTED: Self = Self {
        members: MemberPolicy::StableNested,
    };

    const SCRIPT_METHOD: Self = Self {
        members: MemberPolicy::ScriptMethod,
    };

    const VALUE_METHOD: Self = Self {
        members: MemberPolicy::ValueMethod,
    };
}

fn lower_selected(
    source: &str,
    options: FixtureOptions,
) -> Result<MirProgram, crate::MirBuildError> {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(82),
        ModulePath::from_qualified("assignments"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let body = graph.function_body(declaration.id).expect("main HIR body");
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(ROOT_FUNCTION, body.id)],
    )
    .expect("assignment fixture analysis");
    let body_origin = MirSourceOrigin::body(body.id, body.origin.span);

    let mut targets = CompileTargetSnapshot::builder();
    targets.insert_script_function(
        declaration.id,
        body.id,
        function_descriptor(parameter_targets(&graph, body)),
        body_origin,
    )?;
    if matches!(options.members, MemberPolicy::StableNested) {
        insert_stable_nested_descriptors(&mut targets, body_origin)?;
    }
    if matches!(
        options.members,
        MemberPolicy::ScriptMethod | MemberPolicy::ValueMethod
    ) {
        insert_non_call_method_descriptors(&mut targets, options.members, body_origin)?;
    }
    for field in body.expressions.values().filter_map(|expression| {
        let HirExprKind::Field(field) = &expression.kind else {
            return None;
        };
        Some(field)
    }) {
        targets.insert_member(
            ROOT_FUNCTION,
            field.expression,
            member_target(options.members, field),
            expression_origin(body, field.expression),
        )?;
    }
    let targets = targets.build()?;
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(ROOT_FUNCTION),
        body.id,
        analysis.view(ROOT_FUNCTION).expect("root analysis"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )?;

    crate::build_mir(input)
}

fn function_descriptor(parameters: Vec<CompileParameter>) -> CompileFunctionDescriptor {
    CompileFunctionDescriptor {
        id: ROOT_FUNCTION,
        class: CompileFunctionClass::Script,
        canonical_symbol: "assignments::main".to_owned(),
        debug_name: "main".to_owned(),
        signature: CompileSignature {
            parameters,
            positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
            return_contract: None,
            effect: MirEffect::PURE,
        },
        access: CompileFunctionAccess::script(false),
    }
}

fn parameter_targets(graph: &ModuleGraph, body: &vela_hir::body::HirBody) -> Vec<CompileParameter> {
    let bindings = graph
        .bindings_for_body(body.id)
        .expect("assignment fixture bindings");
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

fn member_target(policy: MemberPolicy, field: &HirField) -> CompileMemberTarget {
    match policy {
        MemberPolicy::Dynamic => CompileMemberTarget::Dynamic {
            name: field.name.clone(),
        },
        MemberPolicy::Tuple => CompileMemberTarget::TupleIndex(
            field
                .name
                .parse()
                .expect("tuple fixture uses a numeric projection"),
        ),
        MemberPolicy::StableNested => match field.name.as_str() {
            "outer" => CompileMemberTarget::ScriptField(CompileFieldTarget::RecordSlot {
                type_id: PLAYER_TYPE,
                shape: PLAYER_SHAPE,
                field: PLAYER_OUTER,
            }),
            "inner" => CompileMemberTarget::ScriptField(CompileFieldTarget::RecordSlot {
                type_id: STATS_TYPE,
                shape: STATS_SHAPE,
                field: STATS_INNER,
            }),
            name => panic!("unexpected stable fixture field {name:?}"),
        },
        MemberPolicy::ScriptMethod => CompileMemberTarget::ScriptMethod {
            target: METHOD_TARGET,
            debug_name: "deliberately-not-the-source-name".to_owned(),
        },
        MemberPolicy::ValueMethod => CompileMemberTarget::ValueMethod {
            owner: METHOD_OWNER,
            method: METHOD,
            debug_name: "deliberately-not-the-source-name".to_owned(),
        },
    }
}

fn insert_non_call_method_descriptors(
    targets: &mut crate::CompileTargetSnapshotBuilder,
    policy: MemberPolicy,
    origin: MirSourceOrigin,
) -> Result<(), crate::MirBuildError> {
    targets.insert_type_descriptor(
        CompileTypeDescriptor {
            id: METHOD_OWNER,
            canonical_name: "assignments::MethodOwner".to_owned(),
            runtime_name: "assignments::MethodOwner".to_owned(),
            class: CompileTypeClass::OpaqueExternal,
            shape: None,
            fields: Vec::new(),
            variants: Vec::new(),
        },
        origin,
    )?;
    let signature = CompileSignature {
        parameters: Vec::new(),
        positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
        return_contract: None,
        effect: MirEffect::PURE,
    };
    let class = match policy {
        MemberPolicy::ScriptMethod => {
            targets.insert_function_descriptor(
                CompileFunctionDescriptor {
                    id: METHOD_FUNCTION,
                    class: CompileFunctionClass::Script,
                    canonical_symbol: "assignments::MethodOwner::visible".to_owned(),
                    debug_name: "visible".to_owned(),
                    signature: CompileSignature {
                        parameters: vec![CompileParameter {
                            name: "self".to_owned(),
                            contract: None,
                            default: CompileParameterDefault::Required,
                            origin: None,
                        }],
                        ..signature.clone()
                    },
                    access: CompileFunctionAccess::script(false),
                },
                origin,
            )?;
            targets.insert_script_method_target(METHOD_TARGET, origin)?;
            CompileMethodClass::Script {
                executable: METHOD_TARGET,
                owner_name: "assignments::MethodOwner".to_owned(),
                code_symbol: "assignments::MethodOwner::visible".to_owned(),
            }
        }
        MemberPolicy::ValueMethod => CompileMethodClass::Value,
        MemberPolicy::Dynamic | MemberPolicy::Tuple | MemberPolicy::StableNested => {
            unreachable!("method descriptor helper requires a method policy")
        }
    };
    targets.insert_method_descriptor(
        CompileMethodDescriptor {
            id: METHOD,
            owner: METHOD_OWNER,
            member_name: "visible".to_owned(),
            debug_name: "assignments::MethodOwner::visible".to_owned(),
            class,
            signature,
            access: CompileMethodAccess::script(),
        },
        origin,
    )
}

fn insert_stable_nested_descriptors(
    targets: &mut crate::CompileTargetSnapshotBuilder,
    origin: MirSourceOrigin,
) -> Result<(), crate::MirBuildError> {
    targets.insert_type_descriptor(
        CompileTypeDescriptor {
            id: PLAYER_TYPE,
            canonical_name: "assignments::Player".to_owned(),
            runtime_name: "assignments::Player".to_owned(),
            class: CompileTypeClass::ScriptRecord,
            shape: Some(PLAYER_SHAPE),
            fields: vec![PLAYER_OUTER],
            variants: Vec::new(),
        },
        origin,
    )?;
    targets.insert_field_descriptor(
        CompileFieldDescriptor {
            id: PLAYER_OUTER,
            owner: PLAYER_TYPE,
            variant: None,
            name: "outer".to_owned(),
            contract: None,
            declaration_order: 0,
            access: CompileFieldAccess::script(),
            host_runtime: None,
        },
        origin,
    )?;
    targets.insert_type_descriptor(
        CompileTypeDescriptor {
            id: STATS_TYPE,
            canonical_name: "assignments::Stats".to_owned(),
            runtime_name: "assignments::Stats".to_owned(),
            class: CompileTypeClass::ScriptRecord,
            shape: Some(STATS_SHAPE),
            fields: vec![STATS_INNER],
            variants: Vec::new(),
        },
        origin,
    )?;
    targets.insert_field_descriptor(
        CompileFieldDescriptor {
            id: STATS_INNER,
            owner: STATS_TYPE,
            variant: None,
            name: "inner".to_owned(),
            contract: None,
            declaration_order: 0,
            access: CompileFieldAccess::script(),
            host_runtime: None,
        },
        origin,
    )
}

fn expression_origin(body: &vela_hir::body::HirBody, expression: HirExprId) -> MirSourceOrigin {
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

#[test]
fn assignment_builder_lowers_typed_local_compound_and_returns_the_result() {
    let program = lower_selected(
        "fn main(value: i64) { return value += 2; }",
        FixtureOptions::DYNAMIC,
    )
    .expect("typed local compound assignment");
    assert_eq!(
        program.dump(),
        r#"mir {
  target function#8200 CompileFunctionDescriptor { id: FunctionId(8200), class: Script, canonical_symbol: "assignments::main", debug_name: "main", signature: CompileSignature { parameters: [CompileParameter { name: "value", contract: None, default: Required, origin: None }], positional: ExactOrTrailingDefaults, return_contract: None, effect: MirEffect { may_trap: false, may_allocate: false, script_call: false, dynamic_call: false, global_read: false, host_read: false, host_write: false, host_call: false, reflection_read: false, reflection_write: false, reflection_call: false, emits_event: false, reads_time: false, uses_random: false, reads_io: false, writes_io: false } }, access: CompileFunctionAccess { public: false, reflect_visible: true, reflect_callable: false } }
  fn f0 body h0 owner function#8200 symbol="assignments::main" @82:20..42/h0 {
    param p0: value -> l0 kind=Explicit(HirParamId(0)) contract=None default=None hir=l0 @82:8..13/h0
    local l0: Script(HirLocalId(0)) Primitive(I64) @82:8..13/h0
    temp t0: Primitive(I64) def=s0 @82:38..39/e2
    temp t1: Primitive(I64) def=s1 @82:29..39/e0
    debug dl0: value -> l0 kind=Parameter hir=Some(0) scope=h0 live=[] @82:8..13/h0
    bb0:
      s0: t0 = constant.literal 2i64 [pure] @82:38..39/e2
      s1: t1 = Numeric { operation: Add, kind: I64 } l0, t0 [trap] @82:29..39/e0
      s2: l0 = t1 [pure] @82:29..39/e0
      -> return t1 [pure] @82:22..40/s0
  }
}
"#
    );
}

#[test]
fn assignment_builder_captures_index_target_before_rhs_and_reads_after_rhs() {
    let program = lower_selected(
        "fn main(values, index, rhs) { return values[index] += rhs + 1; }",
        FixtureOptions::DYNAMIC,
    )
    .expect("dynamic indexed compound assignment");
    let statements = only_function(&program)
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let [
        receiver_capture,
        index_capture,
        rhs_capture,
        literal,
        rhs_add,
        current_read,
        compound,
        write,
    ] = statements.as_slice()
    else {
        panic!("unexpected indexed compound MIR:\n{}", program.dump())
    };
    let receiver = assigned_temp_from_local(receiver_capture);
    let index = assigned_temp_from_local(index_capture);
    let rhs = assigned_temp_from_local(rhs_capture);
    let literal = match (&literal.destination, &literal.kind) {
        (
            Some(crate::MirPlace::Temp(temp)),
            MirStatementKind::Assign(crate::MirRvalue::Constant {
                value: crate::MirImmediate::Scalar(ScalarValue::I64(1)),
                provenance: crate::MirConstantProvenance::Literal,
            }),
        ) => *temp,
        other => panic!("rhs literal lost its explicit definition: {other:?}"),
    };
    assert!(matches!(
        &rhs_add.kind,
        MirStatementKind::DynamicBinary {
            left: MirOperand::Temp(temp),
            right: MirOperand::Temp(right),
            ..
        } if *temp == rhs && *right == literal
    ));
    let current = match (&current_read.destination, &current_read.kind) {
        (
            Some(crate::MirPlace::Temp(current)),
            MirStatementKind::Index(MirIndexOperation::Read {
                receiver: MirOperand::Temp(read_receiver),
                index: MirIndexKey::Value(MirOperand::Temp(read_index)),
            }),
        ) if *read_receiver == receiver && *read_index == index => *current,
        other => panic!("target read lost captured receiver/index: {other:?}"),
    };
    let assigned = match (&compound.destination, &compound.kind) {
        (
            Some(crate::MirPlace::Temp(assigned)),
            MirStatementKind::DynamicBinary {
                left: MirOperand::Temp(read),
                ..
            },
        ) if *read == current => *assigned,
        other => panic!("compound op did not consume the post-RHS read: {other:?}"),
    };
    assert!(matches!(
        &write.kind,
        MirStatementKind::Index(MirIndexOperation::Write {
            receiver: MirOperand::Temp(write_receiver),
            index: MirIndexKey::Value(MirOperand::Temp(write_index)),
            value: MirOperand::Temp(write_value),
        }) if *write_receiver == receiver && *write_index == index && *write_value == assigned
    ));
}

fn assigned_temp_from_local(statement: &crate::MirStatement) -> crate::MirTempId {
    match (&statement.destination, &statement.kind) {
        (
            Some(crate::MirPlace::Temp(temp)),
            MirStatementKind::Assign(crate::MirRvalue::Use(MirOperand::Local(_))),
        ) => *temp,
        other => panic!("expected captured local operand, got {other:?}"),
    }
}

#[test]
fn assignment_builder_compound_read_observes_an_aliasing_rhs_write() {
    let program = lower_selected(
        "fn main(values, index) { return values[index] += (values[index] = 100); }",
        FixtureOptions::DYNAMIC,
    )
    .expect("aliasing indexed compound assignment");
    let statements = only_function(&program)
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let receiver = assigned_temp_from_local(statements[0]);
    let index = assigned_temp_from_local(statements[1]);
    let rhs_write = statements
        .iter()
        .position(|statement| {
            matches!(
                statement.kind,
                MirStatementKind::Index(MirIndexOperation::Write { .. })
            )
        })
        .expect("RHS alias write");
    let current_read = statements
        .iter()
        .position(|statement| {
            matches!(
                &statement.kind,
                MirStatementKind::Index(MirIndexOperation::Read {
                    receiver: MirOperand::Temp(read_receiver),
                    index: MirIndexKey::Value(MirOperand::Temp(read_index)),
                }) if *read_receiver == receiver && *read_index == index
            )
        })
        .expect("outer current-value read");
    assert!(rhs_write < current_read, "{}", program.dump());
    assert_eq!(
        statements
            .iter()
            .filter(|statement| matches!(
                statement.kind,
                MirStatementKind::Index(MirIndexOperation::Write { .. })
            ))
            .count(),
        2
    );
}

#[test]
fn assignment_builder_writes_indexed_nested_fields_back_through_the_captured_key() {
    let program = lower_selected(
        "fn main(records, index, rhs) { return records[index].outer.inner = rhs; }",
        FixtureOptions::DYNAMIC,
    )
    .expect("indexed nested field assignment");
    let statements = only_function(&program)
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let receiver = assigned_temp_from_local(statements[0]);
    let index = assigned_temp_from_local(statements[1]);
    assert_eq!(
        statements
            .iter()
            .filter(|statement| matches!(statement.kind, MirStatementKind::WriteField { .. }))
            .count(),
        2
    );
    assert!(matches!(
        &statements.last().expect("indexed root write-back").kind,
        MirStatementKind::Index(MirIndexOperation::Write {
            receiver: MirOperand::Temp(write_receiver),
            index: MirIndexKey::Value(MirOperand::Temp(write_index)),
            ..
        }) if *write_receiver == receiver && *write_index == index
    ));
}

#[test]
fn assignment_builder_stops_when_a_target_component_diverges() {
    let program = lower_selected(
        "fn main(record, rhs) { ({ return record; }).field = rhs; return 0; }",
        FixtureOptions::DYNAMIC,
    )
    .expect("diverging assignment target");
    let function = only_function(&program);

    assert!(
        !function
            .statements()
            .any(|(_, statement)| matches!(statement.kind, MirStatementKind::WriteField { .. }))
    );
    assert!(program.dump().contains("-> return l0 [pure]"));
    assert!(!program.dump().contains("-> return 0i64"));
}

#[test]
fn assignment_builder_writes_nested_stable_fields_back_to_the_root() {
    let source = r#"
struct Stats { inner: i64 }
struct Player { outer: Stats }
fn main(record: Player, rhs: i64) { return record.outer.inner += rhs; }
"#;
    let program = lower_selected(source, FixtureOptions::STABLE_NESTED)
        .expect("stable nested field assignment");
    let kinds = only_function(&program)
        .statements()
        .map(|(_, statement)| &statement.kind)
        .collect::<Vec<_>>();

    assert!(matches!(kinds[0], MirStatementKind::Assign(_)));
    assert!(matches!(kinds[1], MirStatementKind::ReadField { .. }));
    assert!(matches!(kinds[2], MirStatementKind::Assign(_)));
    assert!(matches!(kinds[3], MirStatementKind::ReadField { .. }));
    assert!(matches!(kinds[4], MirStatementKind::Binary { .. }));
    assert!(matches!(kinds[5], MirStatementKind::WriteField { .. }));
    assert!(matches!(kinds[6], MirStatementKind::WriteField { .. }));
    assert!(program.dump().contains(&format!(
        "RecordSlot {{ type_id: TypeId({}), shape: ShapeId({}), field: FieldId({}) }}",
        STATS_TYPE.get(),
        STATS_SHAPE.get(),
        STATS_INNER.get()
    )));
}

#[test]
fn assignment_builder_uses_explicit_dynamic_fields_without_name_fallback() {
    let program = lower_selected(
        "fn main(record, rhs) { return record.outer.inner = rhs; }",
        FixtureOptions::DYNAMIC,
    )
    .expect("dynamic nested field assignment");
    let writes = only_function(&program)
        .statements()
        .filter_map(|(_, statement)| match &statement.kind {
            MirStatementKind::WriteField { target, .. } => Some(target),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        writes,
        vec![
            &crate::MirFieldTarget::Dynamic {
                name: "inner".to_owned()
            },
            &crate::MirFieldTarget::Dynamic {
                name: "outer".to_owned()
            }
        ]
    );
}

#[test]
fn non_call_method_members_remain_exact_name_dynamic_field_reads() {
    for options in [FixtureOptions::SCRIPT_METHOD, FixtureOptions::VALUE_METHOD] {
        let program = lower_selected("fn main(receiver) { return receiver.visible; }", options)
            .expect("non-call method member value");
        let function = only_function(&program);
        let reads = function
            .statements()
            .filter_map(|(_, statement)| match &statement.kind {
                MirStatementKind::ReadField { target, .. } => Some(target),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            reads,
            vec![&crate::MirFieldTarget::Dynamic {
                name: "visible".to_owned(),
            }],
            "{}",
            program.dump()
        );
        assert!(
            !function
                .statements()
                .any(|(_, statement)| matches!(statement.kind, MirStatementKind::Call(_)))
        );
        assert!(!program.dump().contains("deliberately-not-the-source-name"));
    }
}

#[test]
fn assignment_builder_distinguishes_constant_string_and_dynamic_index_keys() {
    let constant = lower_selected(
        r#"fn main(values) { return values["score"]; }"#,
        FixtureOptions::DYNAMIC,
    )
    .expect("constant string index read");
    let dynamic = lower_selected(
        "fn main(values, key) { return values[key]; }",
        FixtureOptions::DYNAMIC,
    )
    .expect("dynamic index read");

    let constant_index = only_function(&constant)
        .statements()
        .find_map(|(_, statement)| match &statement.kind {
            MirStatementKind::Index(MirIndexOperation::Read { index, .. }) => Some(index),
            _ => None,
        })
        .expect("constant index statement");
    let dynamic_index = only_function(&dynamic)
        .statements()
        .find_map(|(_, statement)| match &statement.kind {
            MirStatementKind::Index(MirIndexOperation::Read { index, .. }) => Some(index),
            _ => None,
        })
        .expect("dynamic index statement");
    assert_eq!(
        constant_index,
        &MirIndexKey::ConstantString("score".to_owned())
    );
    assert!(matches!(
        dynamic_index,
        MirIndexKey::Value(MirOperand::Temp(_))
    ));
}

#[test]
fn assignment_builder_lowers_tuple_projection_reads() {
    let read = lower_selected("fn main(pair) { return pair.1; }", FixtureOptions::TUPLE)
        .expect("tuple projection read");
    assert!(
        only_function(&read)
            .statements()
            .any(|(_, statement)| matches!(
                statement.kind,
                MirStatementKind::TupleField { index: 1, .. }
            ))
    );
}
