use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::SourceId;
use vela_def::{FieldId, FunctionId, TypeId, VariantId};
use vela_hir::body::{HirBody, HirPatternKind};
use vela_hir::ids::HirPatternId;
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use crate::{
    CompileFieldAccess, CompileFieldDescriptor, CompileFunctionAccess, CompileFunctionClass,
    CompileFunctionDescriptor, CompileFunctionIdentity, CompileParameter, CompileParameterDefault,
    CompilePatternConstructorTarget, CompilePositionalPolicy, CompileSignature,
    CompileTargetSnapshot, CompileTargetSnapshotBuilder, CompileTypeClass, CompileTypeDescriptor,
    CompileVariantDescriptor, MirEffect, MirLoweringConfig, MirLoweringInput, MirPlace, MirProgram,
    MirSourceNode, MirSourceOrigin, MirStatementKind, MirTerminatorKind,
};

const SOURCE: SourceId = SourceId::new(87);
const FUNCTION: FunctionId = FunctionId::new(8_700);
const STATIC_TYPE: TypeId = TypeId::new(8_701);
const READY_VARIANT: VariantId = VariantId::new(8_703);
const IDLE_VARIANT: VariantId = VariantId::new(8_704);
const VALUE_FIELD: FieldId = FieldId::new(8_705);

fn build(
    source: &str,
    parameters: &[&str],
    configure: impl FnOnce(
        &HirBody,
        &mut CompileTargetSnapshotBuilder,
    ) -> Result<(), crate::MirBuildError>,
) -> MirProgram {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SOURCE,
        ModulePath::from_qualified("patterns_builder"),
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
    .expect("pattern analysis generation");
    let origin = MirSourceOrigin::body(body.id, body.origin.span);
    let mut targets = CompileTargetSnapshot::builder();
    targets
        .insert_script_function(
            declaration.id,
            body.id,
            CompileFunctionDescriptor {
                id: FUNCTION,
                class: CompileFunctionClass::Script,
                canonical_symbol: "patterns_builder::main".to_owned(),
                debug_name: "main".to_owned(),
                signature: CompileSignature {
                    parameters: parameters
                        .iter()
                        .map(|name| CompileParameter {
                            name: (*name).to_owned(),
                            contract: None,
                            default: CompileParameterDefault::Required,
                            origin: None,
                        })
                        .collect(),
                    positional: CompilePositionalPolicy::ExactOrTrailingDefaults,
                    return_contract: None,
                    effect: MirEffect::PURE,
                },
                access: CompileFunctionAccess::script(false),
            },
            origin,
        )
        .expect("root target");
    configure(body, &mut targets).expect("pattern target setup");
    let targets = targets.build().expect("closed pattern targets");
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(FUNCTION),
        body.id,
        analysis.view(FUNCTION).expect("pattern analysis view"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )
    .expect("valid pattern lowering input");
    crate::build_mir(input).expect("pattern fixture should lower")
}

fn pattern_path(body: &HirBody, pattern: HirPatternId) -> Vec<String> {
    let pattern = body.patterns.get(&pattern).expect("fixture pattern");
    let path = match pattern.kind {
        HirPatternKind::Path { path }
        | HirPatternKind::TupleVariant { path, .. }
        | HirPatternKind::RecordVariant { path, .. } => path,
        HirPatternKind::Binding { .. }
        | HirPatternKind::Wildcard
        | HirPatternKind::Literal(_)
        | HirPatternKind::Missing => None,
    }
    .expect("constructor pattern path");
    body.paths
        .get(&path)
        .expect("fixture pattern path")
        .path
        .clone()
}

fn pattern_origin(body: &HirBody, pattern: HirPatternId) -> MirSourceOrigin {
    let pattern = body.patterns.get(&pattern).expect("fixture pattern");
    MirSourceOrigin::pattern(body.id, pattern.id, pattern.origin.span)
}

fn insert_static_enum_layout(
    targets: &mut CompileTargetSnapshotBuilder,
    origin: MirSourceOrigin,
) -> Result<(), crate::MirBuildError> {
    targets.insert_type_descriptor(
        CompileTypeDescriptor {
            id: STATIC_TYPE,
            canonical_name: "patterns_builder::State".to_owned(),
            class: CompileTypeClass::ScriptEnum,
            shape: None,
            fields: Vec::new(),
            variants: vec![READY_VARIANT, IDLE_VARIANT],
        },
        origin,
    )?;
    targets.insert_variant_descriptor(
        CompileVariantDescriptor {
            id: READY_VARIANT,
            owner: STATIC_TYPE,
            name: "Ready".to_owned(),
            fields: vec![VALUE_FIELD],
            declaration_order: 0,
        },
        origin,
    )?;
    targets.insert_variant_descriptor(
        CompileVariantDescriptor {
            id: IDLE_VARIANT,
            owner: STATIC_TYPE,
            name: "Idle".to_owned(),
            fields: Vec::new(),
            declaration_order: 1,
        },
        origin,
    )?;
    targets.insert_field_descriptor(
        CompileFieldDescriptor {
            id: VALUE_FIELD,
            owner: STATIC_TYPE,
            variant: Some(READY_VARIANT),
            name: "value".to_owned(),
            contract: None,
            declaration_order: 0,
            access: CompileFieldAccess::script(),
            host_runtime: None,
        },
        origin,
    )
}

fn only_function(program: &MirProgram) -> &crate::MirFunction {
    let functions = program.functions().collect::<Vec<_>>();
    let [(_, function)] = functions.as_slice() else {
        panic!("expected one MIR function, got {}", functions.len())
    };
    function
}

#[test]
fn mir_builder_match_literal_order_and_catch_all_snapshot() {
    let program = build(
        "fn main(value) { return match value { 1 => 10, _ => 20 }; }",
        &["value"],
        |_, _| Ok(()),
    );
    assert_eq!(
        program.dump(),
        r#"mir {
  target function#8700 CompileFunctionDescriptor { id: FunctionId(8700), class: Script, canonical_symbol: "patterns_builder::main", debug_name: "main", signature: CompileSignature { parameters: [CompileParameter { name: "value", contract: None, default: Required, origin: None }], positional: ExactOrTrailingDefaults, return_contract: None, effect: MirEffect { may_trap: false, may_allocate: false, script_call: false, dynamic_call: false, global_read: false, host_read: false, host_write: false, host_call: false, reflection_read: false, reflection_write: false, reflection_call: false, emits_event: false, reads_time: false, uses_random: false, reads_io: false, writes_io: false } }, access: CompileFunctionAccess { public: false, reflect_visible: true, reflect_callable: false } }
  fn f0 body h0 owner function#8700 symbol="patterns_builder::main" @87:15..59/h0 {
    param p0: value -> l0 kind=Explicit(HirParamId(0)) contract=None default=None hir=l0 @87:8..13/h0
    local l0: Script(HirLocalId(0)) Dynamic @87:8..13/h0
    local l1: Synthetic Primitive(I64) @87:24..56/e0
    temp t0: Dynamic def=s0 @87:30..35/e1
    temp t1: Primitive(Bool) def=s1 @87:38..39/p0
    debug dl0: value -> l0 kind=Parameter hir=Some(0) scope=h0 live=[] @87:8..13/h0
    safepoint sp0: live={} @87:38..39/p0
    bb0:
      s0: t0 = l0 [pure] @87:30..35/e1
      s1: t1 = dyn.Equal t0, 1i64 [trap|alloc|dynamic-call, sp0] @87:38..39/p0
      -> branch t1 -> bb3, bb2 [pure] @87:38..39/p0
    bb1:
      -> return l1 [pure] @87:17..57/s0
    bb2:
      s3: l1 = 20i64 [pure] @87:52..54/e3
      -> jump bb1 [pure] @87:24..56/e0
    bb3:
      s2: l1 = 10i64 [pure] @87:43..45/e2
      -> jump bb1 [pure] @87:24..56/e0
  }
}
"#
    );
}

#[test]
fn mir_builder_non_exhaustive_match_expression_assigns_unit_on_miss() {
    let program = build(
        "fn main(value) { return match value { 1 => 10 }; }",
        &["value"],
        |_, _| Ok(()),
    );
    let function = only_function(&program);
    let unmatched = function.blocks().find_map(|(_, block)| {
        block.statements().iter().find_map(|statement| {
            let statement = function.statement(*statement)?;
            matches!(
                statement.kind,
                MirStatementKind::Assign(crate::MirRvalue::Use(crate::MirOperand::Immediate(
                    crate::MirImmediate::Unit
                )))
            )
            .then_some(statement)
        })
    });
    assert!(unmatched.is_some(), "{}", program.dump());
}

#[test]
fn mir_builder_match_tuple_predicate_precedes_the_success_binding_guard() {
    let program = build(
        "fn main(value) { return match value { (left, right) => left, _ => 0 }; }",
        &["value"],
        |_, _| Ok(()),
    );
    let function = only_function(&program);
    let dump = program.dump();

    assert!(dump.contains("pattern.tuple-arity"), "{dump}");
    assert_eq!(
        function
            .guards()
            .filter(|(_, guard)| matches!(
                guard.assumption,
                crate::MirGuardAssumption::TupleArity { arity: 2 }
            ))
            .count(),
        1,
        "{dump}"
    );
    let predicate = function
        .statements()
        .find_map(|(statement, data)| {
            matches!(
                data.kind,
                MirStatementKind::Assign(crate::MirRvalue::PatternPredicate(
                    crate::MirPatternPredicate::TupleArity { .. }
                ))
            )
            .then_some(statement)
        })
        .expect("tuple mismatch predicate");
    let guard = function
        .statements()
        .find_map(|(statement, data)| {
            matches!(data.kind, MirStatementKind::GuardTrap { .. }).then_some(statement)
        })
        .expect("tuple success-path binding guard");
    assert!(predicate < guard, "{dump}");
    assert!(function.blocks().any(|(_, block)| matches!(
        block.terminator().map(|terminator| &terminator.kind),
        Some(MirTerminatorKind::Branch { else_block, .. })
            if function.block(*else_block).is_some_and(|next| {
                next.statements().iter().any(|statement| {
                    function.statement(*statement).is_some_and(|statement| matches!(
                        statement.kind,
                        MirStatementKind::Assign(crate::MirRvalue::Use(
                            crate::MirOperand::Immediate(crate::MirImmediate::Scalar(_))
                        ))
                    ))
                })
            })
    )));
}

#[test]
fn mir_builder_match_checks_precede_fresh_binding_projections_and_guard() {
    let program = build(
        "fn main(value, gate) { return match value { (Packet(inner), 9) if gate => inner, _ => 0 }; }",
        &["value", "gate"],
        |body, targets| {
            let packet = body
                .patterns
                .values()
                .find(|pattern| {
                    matches!(
                        pattern.kind,
                        HirPatternKind::TupleVariant { path: Some(_), .. }
                    )
                })
                .expect("nested Packet match pattern");
            targets.insert_pattern_constructor(
                FUNCTION,
                packet.id,
                CompilePatternConstructorTarget::DynamicRecord {
                    type_name: "Packet".to_owned(),
                    fields: vec!["0".to_owned()],
                },
                pattern_origin(body, packet.id),
            )
        },
    );
    let function = only_function(&program);
    let dump = program.dump();
    let last_check = function
        .statements()
        .filter_map(|(statement, data)| {
            matches!(
                data.kind,
                MirStatementKind::Assign(crate::MirRvalue::PatternPredicate(_))
                    | MirStatementKind::DynamicBinary {
                        operation: crate::MirDynamicBinaryOp::Equal,
                        ..
                    }
            )
            .then_some(statement)
        })
        .max()
        .expect("complete structural/literal check sequence");
    let binding_read = function
        .statements()
        .find_map(|(statement, data)| {
            matches!(data.kind, MirStatementKind::ReadField { .. }).then_some(statement)
        })
        .expect("binding-only constructor field projection");
    let binding_write = function
        .statements()
        .find_map(|(statement, data)| {
            (matches!(data.destination, Some(MirPlace::Local(_)))
                && matches!(data.origin.node, MirSourceNode::Pattern(_)))
            .then_some(statement)
        })
        .expect("pattern local write");
    assert!(
        last_check < binding_read && binding_read < binding_write,
        "{dump}"
    );

    let tuple_zero_reads = function
        .statements()
        .filter_map(|(statement, data)| {
            matches!(data.kind, MirStatementKind::TupleField { index: 0, .. }).then_some(statement)
        })
        .collect::<Vec<_>>();
    assert_eq!(tuple_zero_reads.len(), 2, "{dump}");
    assert!(
        tuple_zero_reads[0] < last_check && last_check < tuple_zero_reads[1],
        "binding traversal must reproject after every check\n{dump}"
    );

    let gate = function.parameters()[1].storage;
    assert!(function.blocks().any(|(_, block)| {
        block.statements().contains(&binding_write)
            && matches!(
                block.terminator().map(|terminator| &terminator.kind),
                Some(MirTerminatorKind::Branch {
                    condition: crate::MirOperand::Local(local),
                    ..
                }) if *local == gate
            )
    }));
}

#[test]
fn mir_builder_destructuring_let_guards_only_binding_required_tuples() {
    let program = build(
        "fn main(pair) { let (1, (left, 2)) = pair; return left; }",
        &["pair"],
        |_, _| Ok(()),
    );
    let function = only_function(&program);
    let dump = program.dump();
    let tuple_guards = function
        .guards()
        .filter_map(|(_, guard)| match guard.assumption {
            crate::MirGuardAssumption::TupleArity { arity } => Some(arity),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(tuple_guards, [2, 2], "{dump}");
    assert_eq!(dump.matches("guard.trap").count(), 2, "{dump}");
    assert!(
        !dump.contains("pattern.tuple-arity"),
        "let tuple checks must not become non-trapping match predicates\n{dump}"
    );
    assert!(!dump.contains("dyn.Equal"), "{dump}");
    assert!(!dump.contains("-> fail"), "{dump}");

    let ordered = function.statements().collect::<Vec<_>>();
    let binding_statements = ordered
        .iter()
        .filter_map(|(statement, data)| {
            (matches!(data.destination, Some(MirPlace::Local(_)))
                && matches!(data.origin.node, MirSourceNode::Statement(_)))
            .then_some(*statement)
        })
        .collect::<Vec<_>>();
    assert_eq!(binding_statements.len(), 1, "{dump}");
    let binding = binding_statements[0];
    let last_guard = ordered
        .iter()
        .filter(|(_, statement)| matches!(statement.kind, MirStatementKind::GuardTrap { .. }))
        .map(|(statement, _)| *statement)
        .max()
        .expect("nested tuple guards");
    assert!(last_guard < binding, "{dump}");
    assert_eq!(
        ordered
            .iter()
            .filter(|(_, statement)| matches!(statement.kind, MirStatementKind::TupleField { .. }))
            .count(),
        2,
        "literal-only tuple fields must not be projected\n{dump}"
    );
}

#[test]
fn mir_builder_tuple_only_let_needs_no_generic_mismatch_block() {
    let program = build(
        "fn main(value) { let (left, (middle, right)) = value; return right; }",
        &["value"],
        |_, _| Ok(()),
    );
    let function = only_function(&program);
    let dump = program.dump();

    assert_eq!(
        function
            .guards()
            .filter(|(_, guard)| matches!(
                guard.assumption,
                crate::MirGuardAssumption::TupleArity { arity: 2 }
            ))
            .count(),
        2,
        "{dump}"
    );
    assert!(
        function.blocks().all(|(_, block)| !matches!(
            block.terminator().map(|terminator| &terminator.kind),
            Some(MirTerminatorKind::Fail { .. })
        )),
        "tuple arity guards carry their own trapping behavior\n{dump}"
    );
    assert!(!dump.contains("pattern.tuple-arity"), "{dump}");
}

#[test]
fn mir_builder_nonbinding_literal_let_is_a_noop() {
    let program = build(
        "fn main(value) { let 1 = value; return value; }",
        &["value"],
        |_, _| Ok(()),
    );
    let dump = program.dump();

    assert!(!dump.contains("dyn.Equal"), "{dump}");
    assert!(!dump.contains("-> fail"), "{dump}");
    assert_eq!(only_function(&program).statements().count(), 0, "{dump}");
    assert!(
        only_function(&program).guards().all(|(_, guard)| !matches!(
            guard.assumption,
            crate::MirGuardAssumption::TupleArity { .. }
        )),
        "a literal let pattern does not introduce a runtime match\n{dump}"
    );
}

#[test]
fn mir_builder_constructor_let_projects_bindings_without_matching_the_tag() {
    let program = build(
        "fn main(value) { let (Packet(inner), _) = value; return inner; }",
        &["value"],
        |body, targets| {
            let packet = body
                .patterns
                .values()
                .find(|pattern| {
                    matches!(
                        pattern.kind,
                        HirPatternKind::TupleVariant { path: Some(_), .. }
                    )
                })
                .expect("nested Packet pattern");
            targets.insert_pattern_constructor(
                FUNCTION,
                packet.id,
                CompilePatternConstructorTarget::DynamicRecord {
                    type_name: "Packet".to_owned(),
                    fields: vec!["0".to_owned()],
                },
                pattern_origin(body, packet.id),
            )
        },
    );
    let function = only_function(&program);
    let dump = program.dump();

    assert_eq!(
        function
            .guards()
            .filter(|(_, guard)| matches!(
                guard.assumption,
                crate::MirGuardAssumption::TupleArity { arity: 2 }
            ))
            .count(),
        1,
        "{dump}"
    );
    assert!(dump.contains("field.read"), "{dump}");
    assert!(dump.contains("Dynamic { name: \"0\" }"), "{dump}");
    assert!(!dump.contains("pattern.record.dynamic"), "{dump}");
    assert!(!dump.contains("-> fail"), "{dump}");
    assert!(function.statements().any(|(_, statement)| {
        matches!(statement.destination, Some(MirPlace::Local(_)))
            && matches!(statement.origin.node, MirSourceNode::Statement(_))
    }));
}

#[test]
fn mir_builder_lowers_static_variant_patterns_and_guard_binding_visibility() {
    let source = r#"
enum State { Ready { value }, Idle }
fn main(state, gate) {
    return match state {
        State::Ready { value } if gate => value,
        State::Idle => 0,
        _ => 9,
    };
}
"#;
    let program = build(source, &["state", "gate"], |body, targets| {
        let origin = MirSourceOrigin::body(body.id, body.origin.span);
        insert_static_enum_layout(targets, origin)?;
        for pattern in body.patterns.values().filter(|pattern| {
            matches!(
                pattern.kind,
                HirPatternKind::Path { path: Some(_) }
                    | HirPatternKind::RecordVariant { path: Some(_), .. }
            )
        }) {
            let path = pattern_path(body, pattern.id);
            let target = match path.last().map(String::as_str) {
                Some("Ready") => CompilePatternConstructorTarget::Variant {
                    type_id: STATIC_TYPE,
                    variant: READY_VARIANT,
                    fields: vec![VALUE_FIELD],
                },
                Some("Idle") => CompilePatternConstructorTarget::Variant {
                    type_id: STATIC_TYPE,
                    variant: IDLE_VARIANT,
                    fields: Vec::new(),
                },
                other => panic!("unexpected static pattern {other:?}"),
            };
            targets.insert_pattern_constructor(
                FUNCTION,
                pattern.id,
                target,
                pattern_origin(body, pattern.id),
            )?;
        }
        Ok(())
    });
    let dump = program.dump();

    assert!(
        dump.contains("pattern.variant-shape")
            && dump.contains("type#8701 variant#8703")
            && dump.contains("type#8701 variant#8704"),
        "{dump}"
    );
    assert!(
        dump.contains("field.read") && dump.contains("VariantSlot"),
        "{dump}"
    );
    let function = only_function(&program);
    let gate = function.parameters()[1].storage;
    let (_, guard_block, next_arm) = function
        .blocks()
        .find_map(
            |(block, data)| match data.terminator().map(|term| &term.kind) {
                Some(MirTerminatorKind::Branch {
                    condition: crate::MirOperand::Local(local),
                    else_block,
                    ..
                }) if *local == gate => Some((block, data, *else_block)),
                _ => None,
            },
        )
        .expect("guard branch");
    assert!(
        guard_block.statements().iter().any(|statement| {
            function.statement(*statement).is_some_and(|statement| {
                matches!(statement.destination, Some(MirPlace::Local(_)))
                    && matches!(statement.origin.node, MirSourceNode::Pattern(_))
            })
        }),
        "pattern binding must be visible before its guard\n{dump}"
    );
    assert!(
        function
            .block(next_arm)
            .expect("next arm block")
            .statements()
            .iter()
            .any(|statement| {
                function.statement(*statement).is_some_and(|statement| {
                    matches!(
                        statement.kind,
                        MirStatementKind::Assign(crate::MirRvalue::PatternPredicate(
                            crate::MirPatternPredicate::VariantShape {
                                variant: IDLE_VARIANT,
                                ..
                            }
                        ))
                    )
                })
            }),
        "a false guard must continue with the next ordered arm\n{dump}"
    );
}

#[test]
fn mir_builder_consumes_dynamic_record_and_variant_pattern_targets() {
    let source = r#"
fn main(value) {
    return match value {
        Packet { payload } => payload,
        External::Ready(inner) => inner,
        _ => 0,
    };
}
"#;
    let program = build(source, &["value"], |body, targets| {
        for pattern in body.patterns.values().filter(|pattern| {
            matches!(
                pattern.kind,
                HirPatternKind::TupleVariant { path: Some(_), .. }
                    | HirPatternKind::RecordVariant { path: Some(_), .. }
            )
        }) {
            let path = pattern_path(body, pattern.id);
            let target = match path.as_slice() {
                [packet] if packet == "Packet" => CompilePatternConstructorTarget::DynamicRecord {
                    type_name: packet.clone(),
                    fields: vec!["payload".to_owned()],
                },
                [owner, variant] if owner == "External" && variant == "Ready" => {
                    CompilePatternConstructorTarget::DynamicVariant {
                        owner_name: owner.clone(),
                        variant_name: variant.clone(),
                        fields: vec!["0".to_owned()],
                    }
                }
                other => panic!("unexpected dynamic pattern {other:?}"),
            };
            targets.insert_pattern_constructor(
                FUNCTION,
                pattern.id,
                target,
                pattern_origin(body, pattern.id),
            )?;
        }
        Ok(())
    });
    let dump = program.dump();

    assert!(
        dump.contains("pattern.record.dynamic")
            && dump.contains("type=\"Packet\"")
            && dump.contains("fields=[\"payload\"]"),
        "{dump}"
    );
    assert!(
        dump.contains("pattern.variant.dynamic")
            && dump.contains("owner=\"External\"")
            && dump.contains("variant=\"Ready\"")
            && dump.contains("fields=[\"0\"]"),
        "{dump}"
    );
    assert!(dump.matches("field.read").count() >= 2, "{dump}");
}

#[test]
fn mir_builder_refutable_index_and_value_patterns_advance_the_source_index() {
    let source = r#"
fn main(values) {
    for 1, Pair(left, right) in values { return left; }
    return 0;
}
"#;
    let program = build(source, &["values"], |body, targets| {
        let pattern = body
            .patterns
            .values()
            .find(|pattern| {
                matches!(
                    pattern.kind,
                    HirPatternKind::TupleVariant { path: Some(_), .. }
                )
            })
            .expect("Pair pattern");
        targets.insert_pattern_constructor(
            FUNCTION,
            pattern.id,
            CompilePatternConstructorTarget::DynamicRecord {
                type_name: "Pair".to_owned(),
                fields: vec!["0".to_owned(), "1".to_owned()],
            },
            pattern_origin(body, pattern.id),
        )
    });
    let dump = program.dump();

    let function = only_function(&program);
    let statements = function
        .statements()
        .map(|(_, statement)| &statement.kind)
        .collect::<Vec<_>>();
    let index_capture = statements
        .iter()
        .position(|kind| {
            matches!(
                kind,
                MirStatementKind::Assign(crate::MirRvalue::Use(crate::MirOperand::Local(_)))
            )
        })
        .expect("source index snapshot");
    let increment = statements
        .iter()
        .position(|kind| {
            matches!(
                kind,
                MirStatementKind::Binary {
                    operation: crate::MirBinaryOp::Numeric {
                        operation: crate::MirNumericBinaryOp::Add,
                        ..
                    },
                    ..
                }
            )
        })
        .expect("index increment");
    let index_test = statements
        .iter()
        .position(|kind| {
            matches!(
                kind,
                MirStatementKind::DynamicBinary {
                    operation: crate::MirDynamicBinaryOp::Equal,
                    ..
                }
            )
        })
        .expect("refutable index test");
    let value_test = statements
        .iter()
        .position(|kind| {
            matches!(
                kind,
                MirStatementKind::Assign(crate::MirRvalue::PatternPredicate(
                    crate::MirPatternPredicate::DynamicRecord { .. }
                ))
            )
        })
        .expect("refutable value test");
    assert!(
        index_capture < increment && increment < index_test && index_test < value_test,
        "{dump}"
    );

    let header = function
        .blocks()
        .find_map(|(block, data)| {
            matches!(
                data.terminator().map(|term| &term.kind),
                Some(MirTerminatorKind::IteratorNext { .. })
            )
            .then_some(block)
        })
        .expect("iterator header");
    let mismatch_edges = function
        .blocks()
        .filter(|(_, block)| {
            matches!(
                block.terminator().map(|term| &term.kind),
                Some(MirTerminatorKind::Branch { else_block, .. }) if *else_block == header
            )
        })
        .count();
    assert!(mismatch_edges >= 2, "{dump}");
}
