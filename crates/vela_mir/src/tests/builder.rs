use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_analysis::literals::LiteralPrimitiveContext;
use vela_common::SourceId;
use vela_def::FunctionId;
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use crate::{
    CompileFunctionAccess, CompileFunctionClass, CompileFunctionDescriptor,
    CompileFunctionIdentity, CompileParameter, CompileParameterDefault, CompilePositionalPolicy,
    CompileSignature, CompileTargetSnapshot, MirEffect, MirLoweringConfig, MirLoweringInput,
    MirSourceOrigin,
};

fn build(source: &str) -> crate::MirProgram {
    build_with_parameters(source, Vec::new())
}

fn build_with_parameters(source: &str, parameters: Vec<CompileParameter>) -> crate::MirProgram {
    try_build_with_parameters_and_contexts(source, parameters, &[])
        .expect("supported function should lower")
}

fn build_with_contexts(
    source: &str,
    parameters: Vec<CompileParameter>,
    contexts: &[(&str, LiteralPrimitiveContext)],
) -> crate::MirProgram {
    try_build_with_parameters_and_contexts(source, parameters, contexts)
        .expect("supported function should lower")
}

fn try_build_with_parameters_and_contexts(
    source: &str,
    parameters: Vec<CompileParameter>,
    contexts: &[(&str, LiteralPrimitiveContext)],
) -> Result<crate::MirProgram, crate::MirBuildError> {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(70),
        ModulePath::from_qualified("builder"),
        source,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);

    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let body = graph.function_body(declaration.id).expect("main HIR body");
    let function = FunctionId::new(700);
    let origin = MirSourceOrigin::body(body.id, body.origin.span);
    let contexts = contexts.iter().map(|(text, context)| {
        let matches = body
            .expressions
            .values()
            .filter(|expression| {
                let span = expression.origin.span;
                source.get(span.start as usize..span.end as usize) == Some(*text)
            })
            .map(|expression| expression.id)
            .collect::<Vec<_>>();
        let [expression] = matches.as_slice() else {
            panic!("expected one expression `{text}`, got {matches:?}");
        };
        (*expression, *context)
    });
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(function, body.id).with_literal_contexts(contexts)],
    )
    .expect("builder analysis");
    let mut targets = CompileTargetSnapshot::builder();
    targets
        .insert_script_function(
            declaration.id,
            body.id,
            CompileFunctionDescriptor {
                id: function,
                class: CompileFunctionClass::Script,
                canonical_symbol: "builder::main".to_owned(),
                debug_name: "main".to_owned(),
                signature: CompileSignature {
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
            emit_debug_locals: true,
            compute_liveness: false,
        },
    )
    .expect("valid builder input");
    crate::build_mir(input)
}

fn required_parameter(name: &str) -> CompileParameter {
    CompileParameter {
        name: name.to_owned(),
        contract: None,
        default: CompileParameterDefault::Required,
        origin: None,
    }
}

#[test]
fn mir_builder_maps_parameters_and_local_uses_to_logical_storage() {
    let program = build_with_parameters(
        "fn main(value) { let copy = value; return copy; }",
        vec![CompileParameter {
            name: "value".to_owned(),
            contract: None,
            default: CompileParameterDefault::Required,
            origin: None,
        }],
    );

    assert_eq!(program.defined_len(), 1);
    assert_eq!(
        program.dump(),
        r#"mir {
  target function#700 CompileFunctionDescriptor { id: FunctionId(700), class: Script, canonical_symbol: "builder::main", debug_name: "main", signature: CompileSignature { parameters: [CompileParameter { name: "value", contract: None, default: Required, origin: None }], positional: ExactOrTrailingDefaults, return_contract: None, effect: MirEffect { may_trap: false, may_allocate: false, script_call: false, dynamic_call: false, global_read: false, host_read: false, host_write: false, host_call: false, reflection_read: false, reflection_write: false, reflection_call: false, emits_event: false, reads_time: false, uses_random: false, reads_io: false, writes_io: false } }, access: CompileFunctionAccess { public: false, reflect_visible: true, reflect_callable: false } }
  fn f0 body h0 owner function#700 symbol="builder::main" @70:15..49/h0 {
    param p0: value -> l0 kind=Explicit(HirParamId(0)) contract=None default=None hir=l0 @70:8..13/h0
    local l0: Script(HirLocalId(0)) Dynamic @70:8..13/h0
    local l1: Script(HirLocalId(1)) Dynamic @70:21..25/h0
    debug dl0: value -> l0 kind=Parameter hir=Some(0) scope=h0 live=[] @70:8..13/h0
    debug dl1: copy -> l1 kind=Local hir=Some(1) scope=h0 live=[] @70:21..25/h0
    bb0:
      s0: l1 = l0 [pure] @70:17..34/s0
      -> return l1 [pure] @70:35..47/s1
  }
}
"#
    );
}

#[test]
fn mir_builder_lowers_literals_locals_nested_blocks_and_return() {
    let program =
        build("fn main() { let answer = 42; { let enabled = true; enabled; } return answer; }");

    assert_eq!(program.defined_len(), 1);
    assert_eq!(
        program.dump(),
        r#"mir {
  target function#700 CompileFunctionDescriptor { id: FunctionId(700), class: Script, canonical_symbol: "builder::main", debug_name: "main", signature: CompileSignature { parameters: [], positional: ExactOrTrailingDefaults, return_contract: None, effect: MirEffect { may_trap: false, may_allocate: false, script_call: false, dynamic_call: false, global_read: false, host_read: false, host_write: false, host_call: false, reflection_read: false, reflection_write: false, reflection_call: false, emits_event: false, reads_time: false, uses_random: false, reads_io: false, writes_io: false } }, access: CompileFunctionAccess { public: false, reflect_visible: true, reflect_callable: false } }
  fn f0 body h0 owner function#700 symbol="builder::main" @70:10..78/h0 {
    local l0: Script(HirLocalId(0)) Primitive(I64) @70:16..22/h0
    local l1: Script(HirLocalId(1)) Primitive(Bool) @70:35..42/h0
    temp t0: Primitive(I64) def=s0 @70:25..27/e0
    temp t1: Primitive(Bool) def=s2 @70:45..49/e1
    debug dl0: answer -> l0 kind=Local hir=Some(0) scope=h0 live=[] @70:16..22/h0
    debug dl1: enabled -> l1 kind=Local hir=Some(1) scope=h1 live=[] @70:35..42/h0
    bb0:
      s0: t0 = constant.literal 42i64 [pure] @70:25..27/e0
      s1: l0 = t0 [pure] @70:12..28/s0
      s2: t1 = constant.literal true [pure] @70:45..49/e1
      s3: l1 = t1 [pure] @70:31..50/s2
      -> return l0 [pure] @70:62..76/s4
  }
}
"#
    );
}

#[test]
fn mir_builder_materializes_heap_literals_at_explicit_safepoints() {
    let program = build("fn main() { let greeting = \"hello\"; return greeting; }");

    assert_eq!(program.defined_len(), 1);
    assert_eq!(
        program.dump(),
        r#"mir {
  target function#700 CompileFunctionDescriptor { id: FunctionId(700), class: Script, canonical_symbol: "builder::main", debug_name: "main", signature: CompileSignature { parameters: [], positional: ExactOrTrailingDefaults, return_contract: None, effect: MirEffect { may_trap: false, may_allocate: false, script_call: false, dynamic_call: false, global_read: false, host_read: false, host_write: false, host_call: false, reflection_read: false, reflection_write: false, reflection_call: false, emits_event: false, reads_time: false, uses_random: false, reads_io: false, writes_io: false } }, access: CompileFunctionAccess { public: false, reflect_visible: true, reflect_callable: false } }
  fn f0 body h0 owner function#700 symbol="builder::main" @70:10..54/h0 {
    local l0: Script(HirLocalId(0)) Primitive(String) @70:16..24/h0
    temp t0: Primitive(String) def=s0 @70:27..34/e0
    debug dl0: greeting -> l0 kind=Local hir=Some(0) scope=h0 live=[] @70:16..24/h0
    safepoint sp0: live={} @70:27..34/e0
    bb0:
      s0: t0 = const.materialize "hello" [trap|alloc, sp0] @70:27..34/e0
      s1: l0 = t0 [pure] @70:12..35/s0
      -> return l0 [pure] @70:36..52/s1
  }
}
"#
    );
}

#[test]
fn mir_builder_lowers_proven_scalar_operators_in_left_to_right_order() {
    let program = build_with_parameters(
        r#"fn main(left: i32, right: i32) {
    let minimum = -128i8;
    let negated = -left;
    let inverted = !(left < right);
    let span = left..=right;
    return (left + 1i32) * (right - 2i32);
}"#,
        vec![required_parameter("left"), required_parameter("right")],
    );

    assert_eq!(
        program.dump(),
        r#"mir {
  target function#700 CompileFunctionDescriptor { id: FunctionId(700), class: Script, canonical_symbol: "builder::main", debug_name: "main", signature: CompileSignature { parameters: [CompileParameter { name: "left", contract: None, default: Required, origin: None }, CompileParameter { name: "right", contract: None, default: Required, origin: None }], positional: ExactOrTrailingDefaults, return_contract: None, effect: MirEffect { may_trap: false, may_allocate: false, script_call: false, dynamic_call: false, global_read: false, host_read: false, host_write: false, host_call: false, reflection_read: false, reflection_write: false, reflection_call: false, emits_event: false, reads_time: false, uses_random: false, reads_io: false, writes_io: false } }, access: CompileFunctionAccess { public: false, reflect_visible: true, reflect_callable: false } }
  fn f0 body h0 owner function#700 symbol="builder::main" @70:31..193/h0 {
    param p0: left -> l0 kind=Explicit(HirParamId(0)) contract=None default=None hir=l0 @70:8..12/h0
    param p1: right -> l1 kind=Explicit(HirParamId(1)) contract=None default=None hir=l1 @70:19..24/h0
    local l0: Script(HirLocalId(0)) Primitive(I32) @70:8..12/h0
    local l1: Script(HirLocalId(1)) Primitive(I32) @70:19..24/h0
    local l2: Script(HirLocalId(2)) Primitive(I8) @70:41..48/h0
    local l3: Script(HirLocalId(3)) Primitive(I32) @70:67..74/h0
    local l4: Script(HirLocalId(4)) Primitive(Bool) @70:92..100/h0
    local l5: Script(HirLocalId(5)) Range @70:128..132/h0
    temp t0: Primitive(I8) def=s0 @70:51..57/e0
    temp t1: Primitive(I32) def=s2 @70:77..82/e2
    temp t2: Primitive(I32) def=s4 @70:105..117/e6
    temp t3: Primitive(Bool) def=s5 @70:105..117/e6
    temp t4: Primitive(Bool) def=s6 @70:103..118/e4
    temp t5: Primitive(I32) def=s8 @70:135..147/e9
    temp t6: Range def=s9 @70:135..147/e9
    temp t7: Primitive(I32) def=s11 @70:161..172/e14
    temp t8: Primitive(I32) def=s12 @70:168..172/e16
    temp t9: Primitive(I32) def=s13 @70:161..172/e14
    temp t10: Primitive(I32) def=s14 @70:177..189/e18
    temp t11: Primitive(I32) def=s15 @70:185..189/e20
    temp t12: Primitive(I32) def=s16 @70:177..189/e18
    temp t13: Primitive(I32) def=s17 @70:160..190/e12
    debug dl0: left -> l0 kind=Parameter hir=Some(0) scope=h0 live=[] @70:8..12/h0
    debug dl1: right -> l1 kind=Parameter hir=Some(1) scope=h0 live=[] @70:19..24/h0
    debug dl2: minimum -> l2 kind=Local hir=Some(2) scope=h0 live=[] @70:41..48/h0
    debug dl3: negated -> l3 kind=Local hir=Some(3) scope=h0 live=[] @70:67..74/h0
    debug dl4: inverted -> l4 kind=Local hir=Some(4) scope=h0 live=[] @70:92..100/h0
    debug dl5: span -> l5 kind=Local hir=Some(5) scope=h0 live=[] @70:128..132/h0
    bb0:
      s0: t0 = constant.folded-literal -128i8 [pure] @70:51..57/e0
      s1: l2 = t0 [pure] @70:37..58/s0
      s2: t1 = Negate(I32) l0 [trap] @70:77..82/e2
      s3: l3 = t1 [pure] @70:63..83/s1
      s4: t2 = l0 [pure] @70:105..117/e6
      s5: t3 = Compare { operation: Less, kind: I32 } t2, l1 [trap] @70:105..117/e6
      s6: t4 = NotBool t3 [trap] @70:103..118/e4
      s7: l4 = t4 [pure] @70:88..119/s2
      s8: t5 = l0 [pure] @70:135..147/e9
      s9: t6 = range.make t5, l1 inclusive=true [trap] @70:135..147/e9
      s10: l5 = t6 [pure] @70:124..148/s3
      s11: t7 = l0 [pure] @70:161..172/e14
      s12: t8 = constant.literal 1i32 [pure] @70:168..172/e16
      s13: t9 = Numeric { operation: Add, kind: I32 } t7, t8 [trap] @70:161..172/e14
      s14: t10 = l1 [pure] @70:177..189/e18
      s15: t11 = constant.literal 2i32 [pure] @70:185..189/e20
      s16: t12 = Numeric { operation: Subtract, kind: I32 } t10, t11 [trap] @70:177..189/e18
      s17: t13 = Numeric { operation: Multiply, kind: I32 } t9, t12 [trap] @70:160..190/e12
      -> return t13 [pure] @70:153..191/s4
  }
}
"#
    );
}

#[test]
fn mir_builder_lowers_dynamic_contextual_identity_and_range_operators() {
    let program = build_with_contexts(
        r#"fn main(value, other) {
    let negated = -value;
    let inverted = !value;
    let contextual = value + 1;
    let reversed = 2 - value;
    let compared = value == other;
    let same = value === other;
    let span = value..other;
    return contextual;
}"#,
        vec![required_parameter("value"), required_parameter("other")],
        &[
            ("1", LiteralPrimitiveContext::DeferredDynamic),
            ("2", LiteralPrimitiveContext::DeferredDynamic),
        ],
    );

    assert_eq!(
        program.dump(),
        r#"mir {
  target function#700 CompileFunctionDescriptor { id: FunctionId(700), class: Script, canonical_symbol: "builder::main", debug_name: "main", signature: CompileSignature { parameters: [CompileParameter { name: "value", contract: None, default: Required, origin: None }, CompileParameter { name: "other", contract: None, default: Required, origin: None }], positional: ExactOrTrailingDefaults, return_contract: None, effect: MirEffect { may_trap: false, may_allocate: false, script_call: false, dynamic_call: false, global_read: false, host_read: false, host_write: false, host_call: false, reflection_read: false, reflection_write: false, reflection_call: false, emits_event: false, reads_time: false, uses_random: false, reads_io: false, writes_io: false } }, access: CompileFunctionAccess { public: false, reflect_visible: true, reflect_callable: false } }
  fn f0 body h0 owner function#700 symbol="builder::main" @70:22..259/h0 {
    param p0: value -> l0 kind=Explicit(HirParamId(0)) contract=None default=None hir=l0 @70:8..13/h0
    param p1: other -> l1 kind=Explicit(HirParamId(1)) contract=None default=None hir=l1 @70:15..20/h0
    local l0: Script(HirLocalId(0)) Dynamic @70:8..13/h0
    local l1: Script(HirLocalId(1)) Dynamic @70:15..20/h0
    local l2: Script(HirLocalId(2)) Dynamic @70:32..39/h0
    local l3: Script(HirLocalId(3)) Primitive(Bool) @70:58..66/h0
    local l4: Script(HirLocalId(4)) Dynamic @70:85..95/h0
    local l5: Script(HirLocalId(5)) Dynamic @70:117..125/h0
    local l6: Script(HirLocalId(6)) Primitive(Bool) @70:147..155/h0
    local l7: Script(HirLocalId(7)) Primitive(Bool) @70:182..186/h0
    local l8: Script(HirLocalId(8)) Range @70:214..218/h0
    temp t0: Dynamic def=s0 @70:42..48/e0
    temp t1: Primitive(Bool) def=s2 @70:69..75/e2
    temp t2: Dynamic def=s4 @70:98..107/e4
    temp t3: Dynamic def=s6 @70:128..137/e7
    temp t4: Dynamic def=s8 @70:158..172/e10
    temp t5: Primitive(Bool) def=s9 @70:158..172/e10
    temp t6: Dynamic def=s11 @70:189..204/e13
    temp t7: Primitive(Bool) def=s12 @70:189..204/e13
    temp t8: Dynamic def=s14 @70:221..233/e16
    temp t9: Range def=s15 @70:221..233/e16
    debug dl0: value -> l0 kind=Parameter hir=Some(0) scope=h0 live=[] @70:8..13/h0
    debug dl1: other -> l1 kind=Parameter hir=Some(1) scope=h0 live=[] @70:15..20/h0
    debug dl2: negated -> l2 kind=Local hir=Some(2) scope=h0 live=[] @70:32..39/h0
    debug dl3: inverted -> l3 kind=Local hir=Some(3) scope=h0 live=[] @70:58..66/h0
    debug dl4: contextual -> l4 kind=Local hir=Some(4) scope=h0 live=[] @70:85..95/h0
    debug dl5: reversed -> l5 kind=Local hir=Some(5) scope=h0 live=[] @70:117..125/h0
    debug dl6: compared -> l6 kind=Local hir=Some(6) scope=h0 live=[] @70:147..155/h0
    debug dl7: same -> l7 kind=Local hir=Some(7) scope=h0 live=[] @70:182..186/h0
    debug dl8: span -> l8 kind=Local hir=Some(8) scope=h0 live=[] @70:214..218/h0
    safepoint sp0: live={} @70:158..172/e10
    bb0:
      s0: t0 = dyn.Negate l0 [trap] @70:42..48/e0
      s1: l2 = t0 [pure] @70:28..49/s0
      s2: t1 = dyn.Not l0 [trap] @70:69..75/e2
      s3: l3 = t1 [pure] @70:54..76/s1
      s4: t2 = contextual.Add value=l0 literal=DeferredNumericLiteral { kind: Integer, text: "1" } side=Right [trap] @70:98..107/e4
      s5: l4 = t2 [pure] @70:81..108/s2
      s6: t3 = contextual.Subtract value=l0 literal=DeferredNumericLiteral { kind: Integer, text: "2" } side=Left [trap] @70:128..137/e7
      s7: l5 = t3 [pure] @70:113..138/s3
      s8: t4 = l0 [pure] @70:158..172/e10
      s9: t5 = dyn.Equal t4, l1 [trap|alloc|dynamic-call, sp0] @70:158..172/e10
      s10: l6 = t5 [pure] @70:143..173/s4
      s11: t6 = l0 [pure] @70:189..204/e13
      s12: t7 = identity.Equal t6, l1 [trap] @70:189..204/e13
      s13: l7 = t7 [pure] @70:178..205/s5
      s14: t8 = l0 [pure] @70:221..233/e16
      s15: t9 = range.make t8, l1 inclusive=false [trap] @70:221..233/e16
      s16: l8 = t9 [pure] @70:210..234/s6
      -> return l4 [pure] @70:239..257/s7
  }
}
"#
    );
}

#[test]
fn mir_builder_rejects_invalid_contextual_operator_inputs() {
    let invalid_context = try_build_with_parameters_and_contexts(
        "fn main(value) { return value == 1; }",
        vec![required_parameter("value")],
        &[("1", LiteralPrimitiveContext::DeferredDynamic)],
    )
    .expect_err("unsupported deferred literal operations must not choose a fallback");
    assert!(
        invalid_context
            .to_string()
            .contains("deferred numeric literal is attached to an unsupported binary operator")
    );
}
