use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::SourceId;
use vela_def::FunctionId;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};
use vela_package::ModulePath;

use crate::{
    CompileFunctionAccess, CompileFunctionClass, CompileFunctionDescriptor,
    CompileFunctionIdentity, CompileParameter, CompileParameterDefault, CompilePositionalPolicy,
    CompileSignature, CompileTargetSnapshot, MirEffect, MirLoweringConfig, MirLoweringInput,
    MirSourceOrigin,
};

fn build(source: &str, parameter_names: &[&str]) -> crate::MirProgram {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(71),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("control_flow_builder"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let body = graph.function_body(declaration.id).expect("main HIR body");
    let function = FunctionId::new(710);
    let origin = MirSourceOrigin::body(body.id, body.origin.span);
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(function, body.id)],
    )
    .expect("control-flow builder analysis");
    let parameters = parameter_names
        .iter()
        .map(|name| CompileParameter {
            name: (*name).to_owned(),
            contract: None,
            default: CompileParameterDefault::Required,
            origin: None,
        })
        .collect();
    let mut targets = CompileTargetSnapshot::builder();
    targets
        .insert_script_function(
            declaration.id,
            body.id,
            CompileFunctionDescriptor {
                id: function,
                class: CompileFunctionClass::Script,
                canonical_symbol: "control_flow_builder::main".to_owned(),
                debug_name: "main".to_owned(),
                signature: CompileSignature {
                    asyncness: vela_common::CallableAsyncness::Sync,
                    parameters,
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
            emit_debug_locals: false,
            compute_liveness: false,
        },
    )
    .expect("valid builder input");
    crate::build_mir(input).expect("control-flow function should lower")
}

#[test]
fn mir_builder_lowers_if_values_through_one_mutable_join_local() {
    let program = build(
        r#"fn main(condition) {
    let result = if condition {
        let first = 1;
        first
    } else {
        let second = 2;
        second
    };
    return result;
}"#,
        &["condition"],
    );
    assert_eq!(
        program.dump(),
        r#"mir {
  target function#710 CompileFunctionDescriptor { id: FunctionId(710), class: Script, canonical_symbol: "control_flow_builder::main", debug_name: "main", signature: CompileSignature { asyncness: Sync, parameters: [CompileParameter { name: "condition", contract: None, default: Required, origin: None }], positional: ExactOrTrailingDefaults, return_contract: None, effect: MirEffect { may_trap: false, may_allocate: false, script_call: false, dynamic_call: false, state_read: false, state_write: false, host_read: false, host_write: false, host_call: false, reflection_read: false, reflection_write: false, reflection_call: false, emits_event: false, reads_time: false, uses_random: false, reads_io: false, writes_io: false } }, access: CompileFunctionAccess { public: false, reflect_visible: true, reflect_callable: false } }
  fn f0 body h0 owner function#710 symbol="control_flow_builder::main" @71:19..169/h0 {
    param p0: condition -> l0 kind=Explicit(HirParamId(0)) contract=None default=None hir=l0 @71:8..17/h0
    local l0: Script(HirLocalId(0)) Dynamic @71:8..17/h0
    local l1: Script(HirLocalId(1)) Primitive(I64) @71:65..70/h0
    local l2: Script(HirLocalId(2)) Primitive(I64) @71:115..121/h0
    local l3: Script(HirLocalId(3)) Primitive(I64) @71:29..35/h0
    local l4: Synthetic Primitive(I64) @71:38..147/e0
    temp t0: Primitive(I64) def=s0 @71:73..74/e2
    temp t1: Primitive(I64) def=s3 @71:124..125/e4
    bb0:
      -> branch l0 -> bb1, bb2 [pure] @71:41..50/e1
    bb1:
      s0: t0 = constant.literal 1i64 [pure] @71:73..74/e2
      s1: l1 = t0 [pure] @71:61..75/s1
      s2: l4 = l1 [pure] @71:84..89/e3
      -> jump bb3 [pure] @71:38..147/e0
    bb2:
      s3: t1 = constant.literal 2i64 [pure] @71:124..125/e4
      s4: l2 = t1 [pure] @71:111..126/s3
      s5: l4 = l2 [pure] @71:135..141/e5
      -> jump bb3 [pure] @71:38..147/e0
    bb3:
      s6: l3 = l4 [pure] @71:25..148/s0
      -> return l3 [pure] @71:153..167/s5
  }
}
"#
    );
}

#[test]
fn mir_builder_lowers_short_circuit_operands_in_separate_cfg_paths() {
    let program = build(
        r#"fn main(left, right) {
    let lhs = left;
    return lhs && ({ let rhs = right; rhs });
}"#,
        &["left", "right"],
    );
    let dump = program.dump();

    assert!(dump.contains("branch l2 -> bb1, bb2"), "{dump}");
    assert!(dump.contains("l4 = false"), "{dump}");
    assert!(dump.contains("l3 = l1"), "{dump}");
    assert!(dump.contains("l5 = l3"), "{dump}");
    assert!(dump.contains("l4 = truthy l5"), "{dump}");
    assert!(dump.contains("return l4"), "{dump}");

    let rhs_assignment = dump.find("l3 = l1").expect("right operand assignment");
    let branch = dump.find("branch l2").expect("left branch");
    let lhs_assignment = dump.find("l2 = l0").expect("left operand assignment");
    assert!(lhs_assignment < branch && branch < rhs_assignment, "{dump}");
}

#[test]
fn mir_builder_short_circuit_or_skips_the_right_operand_on_truthy_left() {
    let program = build(
        "fn main(left, right) { return left || right; }",
        &["left", "right"],
    );
    let dump = program.dump();

    assert!(dump.contains("branch l0 -> bb2, bb1"), "{dump}");
    assert!(dump.contains("bb1:\n      s1: l2 = truthy l1"), "{dump}");
    assert!(dump.contains("bb2:\n      s0: l2 = true"), "{dump}");
    assert!(dump.contains("bb3:\n      -> return l2"), "{dump}");
}

#[test]
fn mir_builder_marks_an_if_join_unreachable_when_both_arms_return() {
    let program = build(
        r#"fn main(condition) {
    if condition { return 1; } else { return 2; }
    return 3;
}"#,
        &["condition"],
    );
    let dump = program.dump();

    assert!(dump.contains("branch l0 -> bb1, bb2"), "{dump}");
    assert!(
        dump.contains("bb1:\n      s0: t0 = constant.literal 1i64"),
        "{dump}"
    );
    assert!(dump.contains("-> return t0"), "{dump}");
    assert!(
        dump.contains("bb2:\n      s1: t1 = constant.literal 2i64"),
        "{dump}"
    );
    assert!(dump.contains("-> return t1"), "{dump}");
    assert!(dump.contains("bb3:\n      -> unreachable"), "{dump}");
    assert!(!dump.contains("constant.literal 3i64"), "{dump}");
}

#[test]
fn mir_builder_gives_an_untaken_value_if_arm_unit() {
    let program = build(
        "fn main(condition) { return if condition { 7 }; }",
        &["condition"],
    );
    let dump = program.dump();

    assert!(dump.contains("branch l0 -> bb1, bb2"), "{dump}");
    assert!(dump.contains("constant.literal 7i64"), "{dump}");
    assert!(dump.contains("l1 = unit"), "{dump}");
}

#[test]
fn mir_builder_nested_else_if_joins_only_fallthrough_arms() {
    let program = build(
        "fn main(first, second) { return if first { return 1; } else if second { 2 } else { 3 }; }",
        &["first", "second"],
    );
    let dump = program.dump();

    assert!(dump.contains("branch l0 -> bb1, bb2"), "{dump}");
    assert!(
        dump.contains("bb1:\n      s0: t0 = constant.literal 1i64"),
        "{dump}"
    );
    assert!(
        dump.contains("bb2:\n      -> branch l1 -> bb4, bb5"),
        "{dump}"
    );
    assert!(
        dump.contains("bb4:\n      s1: t1 = constant.literal 2i64"),
        "{dump}"
    );
    assert!(
        dump.contains("bb5:\n      s3: t2 = constant.literal 3i64"),
        "{dump}"
    );
    assert!(dump.contains("bb3:\n      -> return l2"), "{dump}");
}

#[test]
fn mir_builder_captures_a_left_local_before_lowering_a_control_flow_rhs() {
    let program = build(
        "fn main(condition) { let value = 1; return value + if condition { 2 } else { 3 }; }",
        &["condition"],
    );
    let dump = program.dump();

    assert!(dump.contains("s2: t1 = l1 [pure]"), "{dump}");
    assert!(dump.contains("branch l0 -> bb1, bb2"), "{dump}");
    assert!(
        dump.contains("bb1:\n      s3: t2 = constant.literal 2i64"),
        "{dump}"
    );
    assert!(
        dump.contains("bb2:\n      s5: t3 = constant.literal 3i64"),
        "{dump}"
    );
    assert!(
        dump.contains("Numeric { operation: Add, kind: I64 } t1, l2"),
        "{dump}"
    );

    let capture = dump.find("s2: t1 = l1").expect("captured left local");
    let branch = dump.find("branch l0").expect("right-hand if branch");
    let addition = dump
        .find("Numeric { operation: Add")
        .expect("final addition");
    assert!(capture < branch && branch < addition, "{dump}");
}
