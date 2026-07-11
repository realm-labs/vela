use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::{SourceId, Span};
use vela_def::FunctionId;
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use crate::{
    CompileFunctionAccess, CompileFunctionClass, CompileFunctionDescriptor,
    CompileFunctionIdentity, CompileParameter, CompileParameterDefault, CompilePositionalPolicy,
    CompileSignature, CompileTargetSnapshot, MirEffect, MirLoweringConfig, MirLoweringInput,
    MirPlace, MirProgram, MirSourceNode, MirSourceOrigin, MirTerminatorKind,
};

const TEST_SOURCE_ID: SourceId = SourceId::new(76);

fn build(source: &str, parameters: &[&str]) -> MirProgram {
    try_build(source, parameters).expect("supported loop should lower")
}

fn try_build(source: &str, parameters: &[&str]) -> Result<MirProgram, crate::MirBuildError> {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        TEST_SOURCE_ID,
        ModulePath::from_qualified("loops_builder"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let body = graph.function_body(declaration.id).expect("main HIR body");
    let function = FunctionId::new(760);
    let origin = MirSourceOrigin::body(body.id, body.origin.span);
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(function, body.id)],
    )
    .expect("loop builder analysis");
    let mut targets = CompileTargetSnapshot::builder();
    targets
        .insert_script_function(
            declaration.id,
            body.id,
            CompileFunctionDescriptor {
                id: function,
                class: CompileFunctionClass::Script,
                canonical_symbol: "loops_builder::main".to_owned(),
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
        .expect("script target");
    let targets = targets.build().expect("closed compile-target snapshot");
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(function),
        body.id,
        analysis.view(function).expect("function analysis"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )
    .expect("valid loop builder input");
    crate::build_mir(input)
}

fn source_text(source: &str, span: Span) -> &str {
    assert_eq!(span.source, TEST_SOURCE_ID);
    &source[span.start as usize..span.end as usize]
}

#[test]
fn mir_builder_lowers_generic_iteration_with_explicit_safepoints_and_exits() {
    let source = r#"fn main(values) {
    for value in values {
        if value { continue; }
        break;
    }
    return 7;
}"#;
    let program = build(source, &["values"]);
    let dump = program.dump();

    assert!(dump.contains("iterator.create l0"), "{dump}");
    assert!(dump.contains("iterator.next t0"), "{dump}");
    assert!(dump.contains("trap|alloc, sp0"), "{dump}");
    assert!(dump.contains("trap|alloc|dynamic-call, sp1"), "{dump}");
    assert!(dump.contains("-> return 7i64"), "{dump}");
    assert!(!dump.contains("<unterminated>"), "{dump}");
}

#[test]
fn mir_builder_uses_proven_i64_range_steps_without_materializing_a_range() {
    let program = build("fn main() { for value in 1..=3 { value; } return 9; }", &[]);
    let dump = program.dump();

    assert_eq!(
        dump,
        r#"mir {
  target function#760 CompileFunctionDescriptor { id: FunctionId(760), class: Script, canonical_symbol: "loops_builder::main", debug_name: "main", signature: CompileSignature { parameters: [], positional: ExactOrTrailingDefaults, return_contract: None, effect: MirEffect { may_trap: false, may_allocate: false, script_call: false, dynamic_call: false, global_read: false, host_read: false, host_write: false, host_call: false, reflection_read: false, reflection_write: false, reflection_call: false, emits_event: false, reads_time: false, uses_random: false, reads_io: false, writes_io: false } }, access: CompileFunctionAccess { public: false, reflect_visible: true, reflect_callable: false } }
  fn f0 body h0 owner function#760 symbol="loops_builder::main" @76:10..53/h0 {
    local l0: Script(HirLocalId(0)) Primitive(I64) @76:16..21/h0
    local l1: Synthetic Primitive(I64) @76:25..30/e0
    local l2: Synthetic Primitive(Bool) @76:25..30/e0
    local l3: Synthetic Primitive(I64) @76:25..30/e0
    debug dl0: value -> l0 kind=LoopBinding hir=Some(0) scope=h1 live=[] @76:16..21/h0
    bb0:
      s0: l1 = 1i64 [pure] @76:25..30/e0
      s1: l2 = false [pure] @76:25..30/e0
      -> jump bb1 [pure] @76:12..41/s0
    bb1:
      -> range.next cursor=l1 end=3i64 exhausted=l2 inclusive=true item=l3 mode=I64Proven -> next bb2, done bb3 [pure] @76:25..30/e0
    bb2:
      s2: l0 = l3 [pure] @76:16..21/p0
      -> jump bb1 [pure] @76:12..41/s0
    bb3:
      -> return 9i64 [pure] @76:42..51/s2
  }
}
"#
    );
}

#[test]
fn mir_builder_marks_dynamic_integer_ranges_and_indexes_from_zero() {
    let source = r#"fn main(start, end) {
    for index, value in start..end {
        if index == value { break; }
        continue;
    }
    return 0;
}"#;
    let program = build(source, &["start", "end"]);
    let dump = program.dump();

    assert!(dump.contains("mode=DynamicInteger"), "{dump}");
    assert!(dump.contains("inclusive=false"), "{dump}");
    assert!(
        dump.contains("Numeric { operation: Add, kind: I64 }"),
        "{dump}"
    );
    assert!(!dump.contains("iterator.create"), "{dump}");
    assert!(!dump.contains("<unterminated>"), "{dump}");

    let function = program
        .functions()
        .next()
        .map(|(_, function)| function)
        .expect("root MIR function");
    let bound_capture_origins = function
        .statements()
        .filter(|(_, statement)| {
            (matches!(statement.destination, Some(MirPlace::Temp(_)))
                && matches!(statement.origin.node, MirSourceNode::Expression(_)))
        })
        .map(|(_, statement)| source_text(source, statement.origin.span))
        .filter(|text| matches!(*text, "start" | "end"))
        .collect::<Vec<_>>();
    assert_eq!(bound_capture_origins, vec!["start", "end"], "{dump}");
}

#[test]
fn mir_builder_nested_loop_control_targets_the_innermost_context() {
    let source = r#"fn main(outer, inner) {
    for outer_value in outer {
        for inner_value in inner {
            if inner_value { continue; }
            break;
        }
        continue;
    }
    return 0;
}"#;
    let program = build(source, &["outer", "inner"]);
    let function = program
        .functions()
        .next()
        .map(|(_, function)| function)
        .expect("root MIR function");
    let iterator_headers = function
        .blocks()
        .filter_map(|(block, data)| match &data.terminator()?.kind {
            MirTerminatorKind::IteratorNext { done, .. } => Some((block, *done)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(iterator_headers.len(), 2, "{}", program.dump());
    let outer_header = iterator_headers[0].0;
    let inner_header = iterator_headers[1].0;
    let inner_done = iterator_headers[1].1;

    let mut continue_targets = Vec::new();
    let mut break_targets = Vec::new();
    for (_, block) in function.blocks() {
        let Some(terminator) = block.terminator() else {
            continue;
        };
        let MirTerminatorKind::Jump(target) = &terminator.kind else {
            continue;
        };
        if !matches!(terminator.origin.node, MirSourceNode::Statement(_)) {
            continue;
        }
        match source_text(source, terminator.origin.span)
            .trim_end_matches(';')
            .trim()
        {
            "continue" => continue_targets.push((terminator.origin.span.start, *target)),
            "break" => break_targets.push(*target),
            _ => {}
        }
    }
    continue_targets.sort_by_key(|(start, _)| *start);
    assert_eq!(
        continue_targets
            .iter()
            .map(|(_, target)| *target)
            .collect::<Vec<_>>(),
        vec![inner_header, outer_header],
        "{}",
        program.dump()
    );
    assert_eq!(break_targets, vec![inner_done], "{}", program.dump());
}

#[test]
fn mir_builder_stops_cleanly_when_loop_iterable_or_body_diverges() {
    let iterable_return = build(
        "fn main(values) { for value in { return 5; values } { value; } return 9; }",
        &["values"],
    );
    let iterable_dump = iterable_return.dump();
    assert!(iterable_dump.contains("-> return 5i64"), "{iterable_dump}");
    assert!(
        !iterable_dump.contains("iterator.create"),
        "{iterable_dump}"
    );
    assert!(!iterable_dump.contains("<unterminated>"), "{iterable_dump}");

    let body_return = build(
        "fn main(values) { for value in values { return value; } return 9; }",
        &["values"],
    );
    let body_dump = body_return.dump();
    assert!(body_dump.contains("iterator.next"), "{body_dump}");
    assert!(body_dump.contains("-> return l"), "{body_dump}");
    assert!(body_dump.contains("-> return 9i64"), "{body_dump}");
    assert!(!body_dump.contains("<unterminated>"), "{body_dump}");
}

#[test]
fn mir_builder_rejects_destructuring_loop_patterns_for_the_pattern_slice() {
    let error = try_build(
        "fn main(values) { for (left, right) in values { left; } }",
        &["values"],
    )
    .expect_err("destructuring belongs to the pattern-lowering slice");

    assert!(
        error
            .to_string()
            .contains("destructuring for-loop pattern is outside the current MIR builder slice"),
        "{error}"
    );
}

#[test]
fn mir_builder_accepts_wildcard_loop_patterns() {
    let program = build(
        "fn main(values) { for _ in values { continue; } return 0; }",
        &["values"],
    );
    let dump = program.dump();

    assert!(dump.contains("iterator.next"), "{dump}");
    assert!(!dump.contains("<unterminated>"), "{dump}");
}
