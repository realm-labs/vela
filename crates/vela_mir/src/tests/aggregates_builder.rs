use vela_analysis::executable::{ExecutableAnalysisGeneration, ExecutableAnalysisInput};
use vela_common::SourceId;
use vela_def::FunctionId;
use vela_hir::body::HirExprKind;
use vela_hir::ids::HirExprId;
use vela_hir::module_graph::{ModuleGraph, ModulePath, ModuleSource};

use super::*;
use crate::{
    CompileCallTarget, CompileFunctionAccess, CompileFunctionClass, CompileFunctionDescriptor,
    CompileFunctionIdentity, CompileParameter, CompileParameterDefault, CompilePositionalPolicy,
    CompileSignature, CompileTargetSnapshot, MirAggregate, MirBuildError, MirEffect,
    MirFunctionOwner, MirLoweringConfig, MirLoweringInput, MirProgram, MirSourceOrigin,
    MirStatementKind, MirTerminator, MirTerminatorKind,
};

const ROOT_FUNCTION: FunctionId = FunctionId::new(8_100);
const SET_FUNCTION: FunctionId = FunctionId::new(8_101);

fn lower(
    source: &str,
    expression_text: &str,
    set_target: bool,
) -> Result<MirProgram, MirBuildError> {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(81),
        ModulePath::from_qualified("aggregates"),
        source,
    ));
    graph.resolve_imports();

    let declaration = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("main declaration");
    let body = graph.function_body(declaration.id).expect("main HIR body");
    let expression = expression_by_text(body, source, expression_text);
    let analysis = ExecutableAnalysisGeneration::from_module_graph(
        &graph,
        [ExecutableAnalysisInput::new(ROOT_FUNCTION, body.id)],
    )
    .expect("aggregate analysis generation");

    let body_origin = MirSourceOrigin::body(body.id, body.origin.span);
    let mut targets = CompileTargetSnapshot::builder();
    targets.insert_script_function(
        declaration.id,
        body.id,
        function_descriptor(
            ROOT_FUNCTION,
            CompileFunctionClass::Script,
            "aggregates::main",
            Vec::new(),
            CompilePositionalPolicy::ExactOrTrailingDefaults,
        ),
        body_origin,
    )?;
    if set_target {
        let source = match &body
            .expression(expression)
            .expect("set call expression")
            .kind
        {
            HirExprKind::Call(call) => {
                let [argument] = call.arguments.as_slice() else {
                    panic!("set fixture must have one argument")
                };
                argument.value.expect("set fixture argument value")
            }
            other => panic!("expected set call, got {other:?}"),
        };
        targets.insert_function_descriptor(
            function_descriptor(
                SET_FUNCTION,
                CompileFunctionClass::Stdlib,
                "set::from_array",
                vec![CompileParameter {
                    name: "values".to_owned(),
                    contract: None,
                    default: CompileParameterDefault::Required,
                    origin: None,
                }],
                CompilePositionalPolicy::RuntimeChecked,
            ),
            body_origin,
        )?;
        targets.insert_call(
            ROOT_FUNCTION,
            expression,
            CompileCallTarget::positional(
                CompileCalleeTarget::SetFromArray {
                    function: SET_FUNCTION,
                    debug_name: "set::from_array".to_owned(),
                },
                vec![source],
            ),
            expression_origin(body, expression),
        )?;
    }
    let targets = targets.build()?;
    let input = MirLoweringInput::new(
        &graph,
        CompileFunctionIdentity::Function(ROOT_FUNCTION),
        body.id,
        analysis
            .view(ROOT_FUNCTION)
            .expect("root executable analysis"),
        &targets,
        MirLoweringConfig {
            emit_debug_locals: false,
            compute_liveness: false,
        },
    )?;

    lower_isolated_expression(input, expression)
}

fn lower_isolated_expression(
    input: MirLoweringInput<'_>,
    expression: HirExprId,
) -> Result<MirProgram, MirBuildError> {
    let body = input
        .graph()
        .body(input.body())
        .expect("validated MIR body");
    let origin = expression_origin(body, expression);
    let owner = MirFunctionOwner::Function(ROOT_FUNCTION);
    let mut program = MirProgram::new(input.targets().target_table().clone());
    let mut builder = FunctionBuilder::new(input, owner)?;
    let value = match builder.lower_aggregate_expression(expression, origin)? {
        Some(value) => value,
        None => builder.lower_expression(expression)?,
    };
    builder.function.set_terminator(
        builder.current_block,
        MirTerminator::new(
            origin,
            MirTerminatorKind::Return(Some(value)),
            MirEffect::PURE,
            None,
        ),
    )?;
    program.add_function(builder.function)?;
    Ok(program)
}

fn expression_by_text(
    body: &vela_hir::body::HirBody,
    source: &str,
    expression_text: &str,
) -> HirExprId {
    let start = source
        .find(expression_text)
        .unwrap_or_else(|| panic!("missing fixture expression {expression_text:?}"));
    let end = start + expression_text.len();
    let matches = body
        .expressions
        .values()
        .filter(|expression| {
            expression.origin.span.start == u32::try_from(start).expect("fixture start")
                && expression.origin.span.end == u32::try_from(end).expect("fixture end")
        })
        .map(|expression| expression.id)
        .collect::<Vec<_>>();
    if matches.is_empty() && expression_text == "()" {
        let units = body
            .expressions
            .values()
            .filter(|expression| matches!(expression.kind, HirExprKind::Unit))
            .map(|expression| expression.id)
            .collect::<Vec<_>>();
        let [expression] = units.as_slice() else {
            panic!("expected one HIR unit expression, got {units:?}")
        };
        return *expression;
    }
    let [expression] = matches.as_slice() else {
        panic!("expected one HIR expression for {expression_text:?}, got {matches:?}")
    };
    *expression
}

fn expression_origin(body: &vela_hir::body::HirBody, expression: HirExprId) -> MirSourceOrigin {
    let expression = body.expression(expression).expect("fixture expression");
    MirSourceOrigin::expression(body.id, expression.id, expression.origin.span)
}

fn function_descriptor(
    id: FunctionId,
    class: CompileFunctionClass,
    symbol: &str,
    parameters: Vec<CompileParameter>,
    positional: CompilePositionalPolicy,
) -> CompileFunctionDescriptor {
    CompileFunctionDescriptor {
        id,
        class,
        canonical_symbol: symbol.to_owned(),
        debug_name: symbol.to_owned(),
        signature: CompileSignature {
            parameters,
            positional,
            return_contract: None,
            effect: MirEffect::PURE,
        },
        access: CompileFunctionAccess::script(false),
    }
}

fn only_function(program: &MirProgram) -> &crate::MirFunction {
    let functions = program.functions().collect::<Vec<_>>();
    let [(_, function)] = functions.as_slice() else {
        panic!("expected one MIR function, got {}", functions.len())
    };
    function
}

#[test]
fn aggregate_builder_keeps_unit_distinct_from_allocated_tuples() {
    let unit = lower("fn main() { return (); }", "()", false).expect("unit expression");
    assert_eq!(only_function(&unit).statements().count(), 0);
    assert!(unit.dump().contains("-> return unit [pure]"));

    let tuple = lower("fn main() { return (1, 2); }", "(1, 2)", false).expect("tuple expression");
    let statements = only_function(&tuple)
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    let [statement] = statements.as_slice() else {
        panic!("tuple should allocate exactly once")
    };
    assert!(matches!(
        &statement.kind,
        MirStatementKind::Allocate(MirAggregate::Tuple(elements)) if elements.len() == 2
    ));
    assert!(statement.safepoint.is_some());
}

#[test]
fn aggregate_builder_preserves_nested_array_map_and_logical_key_order() {
    let expression = r#"([f"first {1}", 2], {"alpha": [3], beta: f"last {4}"})"#;
    let source = format!("fn main() {{ return {expression}; }}");
    let program = lower(&source, expression, false).expect("nested aggregate expression");
    let kinds = only_function(&program)
        .statements()
        .map(|(_, statement)| &statement.kind)
        .collect::<Vec<_>>();

    assert_eq!(kinds.len(), 6);
    assert!(matches!(kinds[0], MirStatementKind::FormatString { .. }));
    assert!(matches!(
        kinds[1],
        MirStatementKind::Allocate(MirAggregate::Array(values)) if values.len() == 2
    ));
    assert!(matches!(
        kinds[2],
        MirStatementKind::Allocate(MirAggregate::Array(values)) if values.len() == 1
    ));
    assert!(matches!(kinds[3], MirStatementKind::FormatString { .. }));
    assert!(matches!(
        kinds[4],
        MirStatementKind::Allocate(MirAggregate::Map(entries))
            if entries.iter().map(|(key, _)| key.as_str()).collect::<Vec<_>>()
                == ["alpha", "beta"]
    ));
    assert!(matches!(
        kinds[5],
        MirStatementKind::Allocate(MirAggregate::Tuple(values)) if values.len() == 2
    ));
}

#[test]
fn aggregate_builder_preserves_interpolation_part_and_value_evaluation_order() {
    let expression = r#"f"left {"middle"} right {9}""#;
    let source = format!("fn main() {{ return {expression}; }}");
    let program = lower(&source, expression, false).expect("interpolated string");
    let statements = only_function(&program)
        .statements()
        .map(|(_, statement)| statement)
        .collect::<Vec<_>>();
    assert_eq!(statements.len(), 2);
    assert!(matches!(
        statements[0].kind,
        MirStatementKind::MaterializeConstant(_)
    ));
    let MirStatementKind::FormatString { parts } = &statements[1].kind else {
        panic!("interpolation should end in one format operation")
    };
    assert!(matches!(&parts[0], crate::MirFormatPart::Text(text) if text == "left "));
    assert!(matches!(
        &parts[1],
        crate::MirFormatPart::Value(crate::MirOperand::Temp(_))
    ));
    assert!(matches!(&parts[2], crate::MirFormatPart::Text(text) if text == " right "));
    assert!(matches!(
        &parts[3],
        crate::MirFormatPart::Value(crate::MirOperand::Immediate(_))
    ));
    assert!(statements[1].safepoint.is_some());
}

#[test]
fn aggregate_builder_requires_the_explicit_set_compile_target() {
    let expression = "set::from_array([1, 2])";
    let source = format!("fn main() {{ return {expression}; }}");
    let error = lower(&source, expression, false)
        .expect_err("a source name must not invent a set-construction target");
    assert!(
        error
            .to_string()
            .contains("call expression has no compile target")
    );

    let program = lower(&source, expression, true).expect("explicit set construction target");
    let kinds = only_function(&program)
        .statements()
        .map(|(_, statement)| &statement.kind)
        .collect::<Vec<_>>();
    assert_eq!(kinds.len(), 2);
    assert!(matches!(
        kinds[0],
        MirStatementKind::Allocate(MirAggregate::Array(values)) if values.len() == 2
    ));
    assert!(matches!(
        kinds[1],
        MirStatementKind::Allocate(MirAggregate::SetFromArray {
            source: crate::MirOperand::Temp(_)
        })
    ));
}
