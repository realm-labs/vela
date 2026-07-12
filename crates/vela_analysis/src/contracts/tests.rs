use std::collections::BTreeMap;
use vela_package::ModulePath;

use vela_common::{PrimitiveTag, SourceId, Span};
use vela_hir::body::{HirExprKind, HirLiteral};
use vela_hir::ids::HirExprId;
use vela_hir::module_graph::{ModuleGraph, ModuleSource};

use super::*;
use crate::facts::AnalysisFacts;
use crate::literals::LiteralPrimitiveContext;

fn function_parameter(name: &str) -> ExpectedContractContext {
    ExpectedContractContext::FunctionParameter {
        name: name.to_owned(),
    }
}

fn check(
    actual: ContractActual,
    expected: TypeFact,
) -> Result<ExpectedContractOutcome, Box<ContractMismatch>> {
    check_expected_contract(actual, expected, function_parameter("value"))
}

#[test]
fn exact_deferred_and_dynamic_actuals_have_distinct_outcomes() {
    assert_eq!(
        check(ContractActual::Exact(TypeFact::I64), TypeFact::I64),
        Ok(ExpectedContractOutcome::Proven)
    );
    assert_eq!(
        check(
            ContractActual::DeferredNumeric(NumericLiteralKind::Integer),
            TypeFact::U8,
        ),
        Ok(ExpectedContractOutcome::Contextualized(TypeFact::U8))
    );
    assert_eq!(
        check(
            ContractActual::DeferredNumeric(NumericLiteralKind::Float),
            TypeFact::F32,
        ),
        Ok(ExpectedContractOutcome::Contextualized(TypeFact::F32))
    );
    assert_eq!(
        check(ContractActual::Dynamic, TypeFact::I64),
        Ok(ExpectedContractOutcome::RequiresRuntimeGuard(TypeFact::I64))
    );

    let expression = HirExprId::new(41);
    let keyed = check_expected_contract_at(
        expression,
        ContractActual::Dynamic,
        TypeFact::I64,
        function_parameter("value"),
    )
    .expect("dynamic values require guards rather than static rejection");
    assert_eq!(keyed.expression(), expression);
    assert_eq!(
        keyed.outcome(),
        &ExpectedContractOutcome::RequiresRuntimeGuard(TypeFact::I64)
    );

    let mismatch = check(
        ContractActual::DeferredNumeric(NumericLiteralKind::Integer),
        TypeFact::STRING,
    )
    .expect_err("an integer literal cannot satisfy String");
    assert_eq!(
        mismatch.actual(),
        &ContractActual::DeferredNumeric(NumericLiteralKind::Integer)
    );
}

#[test]
fn erased_and_parameterized_container_contracts_preserve_guard_policy() {
    let cases = [
        (
            TypeFact::array(TypeFact::Unknown),
            TypeFact::array(TypeFact::I64),
        ),
        (
            TypeFact::map(TypeFact::Unknown, TypeFact::Unknown),
            TypeFact::map(TypeFact::STRING, TypeFact::I64),
        ),
        (
            TypeFact::set(TypeFact::Unknown),
            TypeFact::set(TypeFact::I64),
        ),
        (
            TypeFact::iterator(TypeFact::Unknown),
            TypeFact::iterator(TypeFact::I64),
        ),
        (
            TypeFact::option(TypeFact::Unknown),
            TypeFact::option(TypeFact::I64),
        ),
        (
            TypeFact::result(TypeFact::Unknown, TypeFact::Unknown),
            TypeFact::result(TypeFact::I64, TypeFact::STRING),
        ),
    ];

    for (erased, parameterized) in cases {
        assert_eq!(
            check(ContractActual::Exact(parameterized.clone()), erased.clone(),),
            Ok(ExpectedContractOutcome::Proven)
        );
        assert_eq!(
            check(ContractActual::Exact(erased), parameterized.clone(),),
            Ok(ExpectedContractOutcome::RequiresRuntimeGuard(parameterized))
        );
    }

    assert!(
        check(
            ContractActual::Exact(TypeFact::array(TypeFact::I64)),
            TypeFact::array(TypeFact::STRING),
        )
        .is_err()
    );
}

#[test]
fn nested_erasure_and_sum_variants_preserve_contract_direction() {
    let nested_erased = TypeFact::map(TypeFact::STRING, TypeFact::array(TypeFact::Unknown));
    let nested_exact = TypeFact::map(TypeFact::STRING, TypeFact::array(TypeFact::I64));
    assert_eq!(
        check(
            ContractActual::Exact(nested_exact.clone()),
            nested_erased.clone(),
        ),
        Ok(ExpectedContractOutcome::Proven)
    );
    assert_eq!(
        check(ContractActual::Exact(nested_erased), nested_exact.clone(),),
        Ok(ExpectedContractOutcome::RequiresRuntimeGuard(nested_exact))
    );

    assert_eq!(
        check(
            ContractActual::Exact(TypeFact::option_some(TypeFact::I64)),
            TypeFact::option(TypeFact::I64),
        ),
        Ok(ExpectedContractOutcome::Proven)
    );
    assert_eq!(
        check(
            ContractActual::Exact(TypeFact::result_err(TypeFact::STRING)),
            TypeFact::result(TypeFact::I64, TypeFact::STRING),
        ),
        Ok(ExpectedContractOutcome::Proven)
    );

    assert!(
        check(
            ContractActual::Exact(TypeFact::map(TypeFact::Unknown, TypeFact::I64)),
            TypeFact::map(TypeFact::STRING, TypeFact::STRING),
        )
        .is_err(),
        "a dynamic key cannot hide an independently incompatible value"
    );
}

#[test]
fn erased_and_dynamic_contract_rules_preserve_guard_direction() {
    let erased_function = TypeFact::function(Vec::new(), TypeFact::Unknown);
    assert_eq!(
        check(
            ContractActual::Exact(TypeFact::Closure),
            erased_function.clone(),
        ),
        Ok(ExpectedContractOutcome::Proven)
    );
    assert!(check(ContractActual::Exact(erased_function), TypeFact::Closure).is_err());
    assert_eq!(
        check(ContractActual::Exact(TypeFact::Any), TypeFact::I64),
        Ok(ExpectedContractOutcome::RequiresRuntimeGuard(TypeFact::I64))
    );
    assert_eq!(
        check(ContractActual::Exact(TypeFact::I64), TypeFact::Any),
        Ok(ExpectedContractOutcome::Proven)
    );
}

#[test]
fn mismatch_diagnostics_freeze_every_expected_contract_context() {
    let span = Span::new(SourceId::new(4), 10, 13);
    let cases = [
        (
            ExpectedContractContext::FunctionParameter {
                name: "amount".to_owned(),
            },
            "type contract mismatch for parameter `amount`",
        ),
        (
            ExpectedContractContext::NativeParameter {
                function: "i64::from_i32".to_owned(),
                name: "value".to_owned(),
                index: 0,
            },
            "type contract mismatch for native parameter `i64::from_i32::value`",
        ),
        (
            ExpectedContractContext::TypedLet {
                name: "amount".to_owned(),
            },
            "type contract mismatch for typed local `amount`",
        ),
        (
            ExpectedContractContext::Field {
                name: "amount".to_owned(),
            },
            "type contract mismatch for field `amount`",
        ),
    ];

    for (context, message) in cases {
        let mismatch = check_expected_contract(
            ContractActual::Exact(TypeFact::STRING),
            TypeFact::I64,
            context,
        )
        .expect_err("String cannot satisfy i64");
        let diagnostic = mismatch.to_diagnostic(span);
        assert_eq!(
            diagnostic.code.as_deref(),
            Some(TYPE_CONTRACT_MISMATCH_CODE)
        );
        assert_eq!(diagnostic.message, message);
        assert_eq!(diagnostic.span, Some(span));
        assert_eq!(diagnostic.labels.len(), 1);
        assert_eq!(diagnostic.labels[0].span, span);
        assert_eq!(
            diagnostic.labels[0].message,
            "expected `i64`, found `String`"
        );
    }
}

#[test]
fn hir_keyed_mismatch_preserves_expression_identity_and_span() {
    let graph = graph_with_source(
        SourceId::new(8),
        "fn grant(amount: i64) { return amount; }\nfn main() { return grant(\"x\"); }",
    );
    let expression = find_literal(&graph, |literal| matches!(literal, HirLiteral::String(_)));
    let mismatch = check_expected_contract_at(
        expression,
        ContractActual::Exact(TypeFact::STRING),
        TypeFact::I64,
        function_parameter("amount"),
    )
    .expect_err("String cannot satisfy i64");
    let diagnostic = mismatch
        .to_diagnostic(&graph)
        .expect("stored HIR expression should have an origin");

    assert_eq!(mismatch.expression(), expression);
    assert_eq!(diagnostic.span, graph.expression_span(expression));
    assert_eq!(
        diagnostic.code.as_deref(),
        Some(TYPE_CONTRACT_MISMATCH_CODE)
    );
    assert_eq!(
        diagnostic.message,
        "type contract mismatch for parameter `amount`"
    );
}

#[test]
fn incompatible_literal_context_becomes_type_contract_mismatch() {
    let source = SourceId::new(9);
    let graph = graph_with_source(
        source,
        "fn main() { let amount: String = 1; return amount; }",
    );
    let expression = find_literal(&graph, |literal| matches!(literal, HirLiteral::Integer(_)));
    let mut facts = AnalysisFacts::from_module_graph(&graph);
    facts.resolve_literal_contexts(
        &graph,
        &BTreeMap::from([(
            expression,
            LiteralPrimitiveContext::Expected(PrimitiveTag::String),
        )]),
    );
    let literal = facts.literal(expression).expect("numeric literal fact");
    let error = literal
        .as_ref()
        .expect_err("integer literal should reject String context");
    assert_eq!(error.class(), LiteralErrorClass::IncompatiblePrimitive);
    assert_eq!(error.to_compiler_diagnostic(Span::new(source, 0, 0)), None);
    assert!(facts.literal_diagnostics(&graph).is_empty());

    let mismatch = check_expected_contract_at(
        expression,
        ContractActual::from_literal_result(
            literal,
            ContractActual::DeferredNumeric(NumericLiteralKind::Integer),
        )
        .expect("incompatible literal retains its HIR-derived actual kind"),
        TypeFact::STRING,
        ExpectedContractContext::TypedLet {
            name: "amount".to_owned(),
        },
    )
    .expect_err("literal incompatibility is an expected-contract mismatch");
    let diagnostic = mismatch
        .to_diagnostic(&graph)
        .expect("literal expression should have a source origin");
    assert_eq!(
        diagnostic.code.as_deref(),
        Some(TYPE_CONTRACT_MISMATCH_CODE)
    );
    assert_eq!(
        diagnostic.labels[0].message,
        "expected `String`, found unsuffixed integer literal"
    );
}

#[test]
fn callable_contracts_distinguish_erased_zero_and_exact_arity() {
    let context = function_parameter("callback");
    let erased = ExpectedCallableContract::new(ExpectedCallableKind::Function, None);
    let zero = ExpectedCallableContract::new(ExpectedCallableKind::Function, Some(0));
    let one = ExpectedCallableContract::new(ExpectedCallableKind::Function, Some(1));
    let known_zero = ContractActual::Exact(TypeFact::function(Vec::new(), TypeFact::BOOL));
    let known_one = ContractActual::Exact(TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL));
    let erased_actual = ContractActual::Exact(TypeFact::function(Vec::new(), TypeFact::Unknown));

    assert_eq!(
        check_expected_callable_contract(known_one.clone(), erased, context.clone()),
        Ok(ExpectedContractOutcome::Proven)
    );
    assert_eq!(
        check_expected_callable_contract(known_zero, zero, context.clone()),
        Ok(ExpectedContractOutcome::Proven)
    );
    assert_eq!(
        check_expected_callable_contract(known_one.clone(), one, context.clone()),
        Ok(ExpectedContractOutcome::Proven)
    );
    assert!(check_expected_callable_contract(known_one, zero, context.clone()).is_err());
    assert_eq!(
        check_expected_callable_contract(erased_actual, zero, context),
        Ok(ExpectedContractOutcome::RequiresRuntimeGuard(
            TypeFact::function(Vec::new(), TypeFact::Unknown)
        ))
    );
}

#[test]
fn callable_contracts_preserve_kind_direction_and_dynamic_guards() {
    let context = function_parameter("callback");
    let function = ExpectedCallableContract::new(ExpectedCallableKind::Function, None);
    let closure = ExpectedCallableContract::new(ExpectedCallableKind::Closure, None);

    assert_eq!(
        check_expected_callable_contract(
            ContractActual::Exact(TypeFact::Closure),
            function,
            context.clone(),
        ),
        Ok(ExpectedContractOutcome::Proven)
    );
    assert!(
        check_expected_callable_contract(
            ContractActual::Exact(TypeFact::function(vec![TypeFact::I64], TypeFact::BOOL)),
            closure,
            context.clone(),
        )
        .is_err()
    );
    assert_eq!(
        check_expected_callable_contract(
            ContractActual::Exact(TypeFact::Closure),
            closure,
            context.clone(),
        ),
        Ok(ExpectedContractOutcome::Proven)
    );

    let expression = HirExprId::new(991);
    let validation = check_expected_callable_contract_at(
        expression,
        ContractActual::Dynamic,
        ExpectedCallableContract::new(ExpectedCallableKind::Closure, Some(0)),
        context,
    )
    .expect("dynamic callable requires a runtime guard");
    assert_eq!(validation.expression(), expression);
    assert_eq!(
        validation.outcome(),
        &ExpectedContractOutcome::RequiresRuntimeGuard(TypeFact::Closure)
    );
}

fn graph_with_source(source: SourceId, text: &str) -> ModuleGraph {
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        source,
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified(""),
        text,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    graph
}

fn find_literal(graph: &ModuleGraph, predicate: impl Fn(&HirLiteral) -> bool) -> HirExprId {
    graph
        .bodies()
        .flat_map(|body| body.expressions.values())
        .find_map(|expression| match &expression.kind {
            HirExprKind::Literal(literal) if predicate(literal) => Some(expression.id),
            _ => None,
        })
        .expect("matching literal expression")
}
