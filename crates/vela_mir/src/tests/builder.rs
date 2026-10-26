use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_analysis::literals::LiteralPrimitiveContext;
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
        vela_package::PackageId::anonymous(),
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
        include_str!("../../tests/fixtures/builder/parameter_storage.vela"),
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
        include_str!("../../tests/fixtures/builder/parameter_storage.expected")
    );
}

#[test]
fn mir_builder_lowers_literals_locals_nested_blocks_and_return() {
    let program = build(include_str!(
        "../../tests/fixtures/builder/nested_blocks.vela"
    ));

    assert_eq!(program.defined_len(), 1);
    assert_eq!(
        program.dump(),
        include_str!("../../tests/fixtures/builder/nested_blocks.expected")
    );
}

#[test]
fn mir_builder_materializes_heap_literals_at_explicit_safepoints() {
    let program = build(include_str!(
        "../../tests/fixtures/builder/heap_literal.vela"
    ));

    assert_eq!(program.defined_len(), 1);
    assert_eq!(
        program.dump(),
        include_str!("../../tests/fixtures/builder/heap_literal.expected")
    );
}

#[test]
fn mir_builder_lowers_proven_scalar_operators_in_left_to_right_order() {
    let program = build_with_parameters(
        include_str!("../../tests/fixtures/builder/scalar_operators.vela"),
        vec![required_parameter("left"), required_parameter("right")],
    );

    assert_eq!(
        program.dump(),
        include_str!("../../tests/fixtures/builder/scalar_operators.expected")
    );
}

#[test]
fn mir_builder_lowers_dynamic_contextual_identity_and_range_operators() {
    let program = build_with_contexts(
        include_str!("../../tests/fixtures/builder/dynamic_operators.vela"),
        vec![required_parameter("value"), required_parameter("other")],
        &[
            ("1", LiteralPrimitiveContext::DeferredDynamic),
            ("2", LiteralPrimitiveContext::DeferredDynamic),
        ],
    );

    assert_eq!(
        program.dump(),
        include_str!("../../tests/fixtures/builder/dynamic_operators.expected")
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
