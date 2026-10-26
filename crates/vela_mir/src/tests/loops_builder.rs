use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::{SourceId, Span};
use vela_def::FunctionId;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::ModulePath;

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
        vela_package::PackageId::anonymous(),
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
    assert!(dump.contains("constant.literal 7i64"), "{dump}");
    assert!(!dump.contains("<unterminated>"), "{dump}");
}

#[test]
fn mir_builder_uses_proven_i64_range_steps_without_materializing_a_range() {
    let program = build(
        include_str!("../../tests/fixtures/builder/range_loop.vela"),
        &[],
    );
    let dump = program.dump();

    assert_eq!(
        dump,
        include_str!("../../tests/fixtures/builder/range_loop.expected")
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
    assert!(
        iterable_dump.contains("constant.literal 5i64"),
        "{iterable_dump}"
    );
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
    assert!(body_dump.contains("constant.literal 9i64"), "{body_dump}");
    assert!(!body_dump.contains("<unterminated>"), "{body_dump}");
}

#[test]
fn mir_builder_iterable_exits_target_the_enclosing_loop() {
    for exit in ["break", "continue"] {
        let source = format!(
            "fn main(values) {{ for outer in values {{ for inner in {{ {exit}; values }} {{ inner; }} }} return 9; }}"
        );
        let program = build(&source, &["values"]);
        let dump = program.dump();
        let function = program
            .functions()
            .next()
            .map(|(_, function)| function)
            .expect("root MIR function");
        let headers = function
            .blocks()
            .filter_map(|(id, block)| match &block.terminator()?.kind {
                MirTerminatorKind::IteratorNext { done, .. } => Some((id, *done)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(headers.len(), 1, "{dump}");
        let expected = if exit == "break" {
            headers[0].1
        } else {
            headers[0].0
        };
        let targets = function
            .blocks()
            .filter_map(|(_, block)| {
                let terminator = block.terminator()?;
                let MirTerminatorKind::Jump(target) = &terminator.kind else {
                    return None;
                };
                (source_text(&source, terminator.origin.span)
                    .trim_end_matches(';')
                    .trim()
                    == exit)
                    .then_some(*target)
            })
            .collect::<Vec<_>>();
        assert_eq!(targets, vec![expected], "{dump}");
        assert!(!dump.contains("<unterminated>"), "{dump}");
    }
}

#[test]
fn mir_builder_lowers_destructuring_loop_patterns_before_the_user_body() {
    let program = build(
        "fn main(values) { for (left, right) in values { left; } }",
        &["values"],
    );
    let dump = program.dump();

    assert!(dump.contains("pattern.tuple-arity"), "{dump}");
    assert!(dump.contains("tuple.field"), "{dump}");
    assert!(!dump.contains("<unterminated>"), "{dump}");
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
