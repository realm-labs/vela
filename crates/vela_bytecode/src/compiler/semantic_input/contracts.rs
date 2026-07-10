use std::collections::BTreeMap;

use vela_analysis::contracts::{
    ContractActual, ExpectedCallableContract, ExpectedCallableKind, ExpectedContractContext,
    ExpectedContractOutcome, check_expected_callable_contract, check_expected_callable_contract_at,
    check_expected_contract, check_expected_contract_at,
};
use vela_analysis::literals::{
    LiteralPrimitiveContext, NumericLiteralKind, float_suffix_primitive, integer_suffix_primitive,
};
use vela_analysis::semantic_facts::OperatorTargetFact;
use vela_analysis::type_fact::TypeFact;
use vela_common::{PrimitiveTag, Span};
use vela_def::FunctionId;
use vela_hir::body::{HirBinaryOp, HirExprKind, HirLiteral, HirUnaryOp};
use vela_hir::ids::HirExprId;
use vela_mir::{
    CompileGuardKey, CompileGuardTarget, MirBuildError, MirCallableKind, MirTypeContract,
};

use super::{GenerationBuilder, input_error, registry_input_error};
use crate::compiler::error::CompileResult;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ContractBoundary {
    function: FunctionId,
    expression: HirExprId,
    expected: MirTypeContract,
    context: ExpectedContractContext,
}

impl ContractBoundary {
    pub(super) fn function_parameter(
        function: FunctionId,
        expression: HirExprId,
        expected: MirTypeContract,
        name: String,
    ) -> Self {
        Self {
            function,
            expression,
            expected,
            context: ExpectedContractContext::FunctionParameter { name },
        }
    }

    pub(super) fn native_parameter(
        function: FunctionId,
        expression: HirExprId,
        expected: MirTypeContract,
        display_function: impl Into<String>,
        name: impl Into<String>,
        index: u16,
    ) -> Self {
        Self {
            function,
            expression,
            expected,
            context: ExpectedContractContext::NativeParameter {
                function: display_function.into(),
                name: name.into(),
                index,
            },
        }
    }

    pub(super) fn typed_let(
        function: FunctionId,
        expression: HirExprId,
        expected: MirTypeContract,
        name: String,
    ) -> Self {
        Self {
            function,
            expression,
            expected,
            context: ExpectedContractContext::TypedLet { name },
        }
    }

    pub(super) fn field(
        function: FunctionId,
        expression: HirExprId,
        expected: MirTypeContract,
        name: impl Into<String>,
    ) -> Self {
        Self {
            function,
            expression,
            expected,
            context: ExpectedContractContext::Field { name: name.into() },
        }
    }
}

impl GenerationBuilder<'_, '_> {
    pub(super) fn validate_schema_default_contract(
        &mut self,
        body: vela_hir::ids::HirBodyId,
        expected: &MirTypeContract,
        field: &str,
        span: Span,
    ) -> CompileResult<()> {
        let Some(Some(value)) = self.request.schema_defaults.evaluated_defaults().get(&body) else {
            return Ok(());
        };
        let actual = constant_actual(value);
        let context = ExpectedContractContext::Field {
            name: field.to_owned(),
        };
        let result = match expected {
            MirTypeContract::Callable {
                kind,
                positional_arity,
            } => check_expected_callable_contract(
                actual,
                ExpectedCallableContract::new(
                    match kind {
                        MirCallableKind::Function => ExpectedCallableKind::Function,
                        MirCallableKind::Closure => ExpectedCallableKind::Closure,
                    },
                    *positional_arity,
                ),
                context,
            ),
            expected => {
                check_expected_contract(actual, self.type_fact_for_contract(expected)?, context)
            }
        };
        if let Err(mismatch) = result {
            self.diagnostics.push(mismatch.to_diagnostic(span));
        }
        Ok(())
    }

    pub(super) fn finish_contracts(&mut self) -> CompileResult<()> {
        for (function, _) in self.selected_executable_roots()? {
            let diagnostics = self
                .executable_analysis(function)?
                .literal_diagnostics(self.request.graph);
            for diagnostic in diagnostics {
                if !self.diagnostics.contains(&diagnostic) {
                    self.diagnostics.push(diagnostic);
                }
            }
        }

        let mut checked =
            BTreeMap::<(FunctionId, HirExprId), (MirTypeContract, ExpectedContractContext)>::new();
        for boundary in self.boundaries.clone() {
            let key = (boundary.function, boundary.expression);
            if let Some((expected, context)) = checked.get(&key) {
                if expected == &boundary.expected && context == &boundary.context {
                    continue;
                }
                let origin = self
                    .expression_origin(boundary.expression)
                    .ok_or_else(registry_input_error)?;
                return Err(input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: format!(
                        "expression {:?} has conflicting expected contracts",
                        boundary.expression
                    ),
                }));
            }
            checked.insert(key, (boundary.expected.clone(), boundary.context.clone()));
            let actual = self.contract_actual(boundary.function, boundary.expression)?;
            let validation = match &boundary.expected {
                MirTypeContract::Callable {
                    kind,
                    positional_arity,
                } => check_expected_callable_contract_at(
                    boundary.expression,
                    actual,
                    ExpectedCallableContract::new(
                        match kind {
                            MirCallableKind::Function => ExpectedCallableKind::Function,
                            MirCallableKind::Closure => ExpectedCallableKind::Closure,
                        },
                        *positional_arity,
                    ),
                    boundary.context.clone(),
                ),
                expected => check_expected_contract_at(
                    boundary.expression,
                    actual,
                    self.type_fact_for_contract(expected)?,
                    boundary.context.clone(),
                ),
            };
            match validation {
                Ok(validation) => {
                    if let ExpectedContractOutcome::RequiresRuntimeGuard(_) = validation.outcome() {
                        let origin = self
                            .expression_origin(boundary.expression)
                            .ok_or_else(registry_input_error)?;
                        self.targets
                            .insert_guard(
                                CompileGuardKey::Expression {
                                    function: boundary.function,
                                    expression: boundary.expression,
                                },
                                CompileGuardTarget {
                                    contract: boundary.expected,
                                    debug_name: boundary.context.description(),
                                },
                                origin,
                            )
                            .map_err(input_error)?;
                    }
                }
                Err(mismatch) => {
                    if let Some(diagnostic) = mismatch.to_diagnostic(self.request.graph) {
                        self.diagnostics.push(diagnostic);
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn literal_contexts(
        &self,
    ) -> CompileResult<BTreeMap<FunctionId, BTreeMap<HirExprId, LiteralPrimitiveContext>>> {
        let mut contexts =
            BTreeMap::<FunctionId, BTreeMap<HirExprId, LiteralPrimitiveContext>>::new();
        for (function, root) in self.selected_executable_roots()? {
            self.collect_dynamic_literal_contexts(
                function,
                &self.executable_body_ids(root),
                contexts.entry(function).or_default(),
            )?;
        }
        for boundary in &self.boundaries {
            if numeric_kind(self.request.graph, boundary.expression).is_none() {
                continue;
            }
            let context = match boundary.expected {
                MirTypeContract::Primitive(primitive) => {
                    LiteralPrimitiveContext::Expected(primitive)
                }
                _ => LiteralPrimitiveContext::Expected(PrimitiveTag::Unit),
            };
            let function_contexts = contexts.entry(boundary.function).or_default();
            if let Some(previous) = function_contexts.insert(boundary.expression, context)
                && previous != context
            {
                let origin = self
                    .expression_origin(boundary.expression)
                    .ok_or_else(registry_input_error)?;
                return Err(input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: format!(
                        "expression {:?} has conflicting literal contexts in function #{}",
                        boundary.expression,
                        boundary.function.get()
                    ),
                }));
            }
        }
        Ok(contexts)
    }

    fn collect_dynamic_literal_contexts(
        &self,
        function: FunctionId,
        bodies: &[vela_hir::ids::HirBodyId],
        contexts: &mut BTreeMap<HirExprId, LiteralPrimitiveContext>,
    ) -> CompileResult<()> {
        let analysis = self.executable_analysis(function)?;
        for body in bodies
            .iter()
            .filter_map(|body| self.request.graph.body(*body))
        {
            for expression in body.expressions.values() {
                let HirExprKind::Binary {
                    op:
                        Some(
                            HirBinaryOp::Add
                            | HirBinaryOp::Sub
                            | HirBinaryOp::Mul
                            | HirBinaryOp::Div
                            | HirBinaryOp::Rem
                            | HirBinaryOp::Equal
                            | HirBinaryOp::NotEqual
                            | HirBinaryOp::Less
                            | HirBinaryOp::LessEqual
                            | HirBinaryOp::Greater
                            | HirBinaryOp::GreaterEqual,
                        ),
                    lhs,
                    rhs,
                } = expression.kind
                else {
                    continue;
                };
                if analysis.operator_target(expression.id) != Some(OperatorTargetFact::Dynamic) {
                    continue;
                }
                for operand in lhs.into_iter().chain(rhs) {
                    if numeric_kind(self.request.graph, operand).is_some() {
                        contexts.insert(operand, LiteralPrimitiveContext::DeferredDynamic);
                    }
                }
            }
        }
        Ok(())
    }

    fn contract_actual(
        &self,
        function: FunctionId,
        expression: HirExprId,
    ) -> CompileResult<ContractActual> {
        if let Some(kind) = numeric_kind(self.request.graph, expression) {
            return Ok(ContractActual::DeferredNumeric(kind));
        }
        let Some(record) = self
            .request
            .graph
            .bodies()
            .find_map(|body| body.expression(expression))
        else {
            return Ok(ContractActual::Dynamic);
        };
        let analysis = self.executable_analysis(function)?;
        Ok(match &record.kind {
            HirExprKind::Literal(HirLiteral::Bool(_)) => ContractActual::Exact(TypeFact::BOOL),
            HirExprKind::Literal(HirLiteral::Integer(value)) => {
                ContractActual::Exact(TypeFact::primitive(integer_suffix_primitive(value.suffix)))
            }
            HirExprKind::Literal(HirLiteral::Float(value)) => {
                ContractActual::Exact(TypeFact::primitive(float_suffix_primitive(value.suffix)))
            }
            HirExprKind::Literal(HirLiteral::Char(_)) => ContractActual::Exact(TypeFact::CHAR),
            HirExprKind::Literal(HirLiteral::String(_) | HirLiteral::Interpolated { .. }) => {
                ContractActual::Exact(TypeFact::STRING)
            }
            HirExprKind::Literal(HirLiteral::Bytes(_)) => ContractActual::Exact(TypeFact::BYTES),
            HirExprKind::Unit => ContractActual::Exact(TypeFact::UNIT),
            HirExprKind::Lambda { .. } => ContractActual::Exact(TypeFact::Closure),
            HirExprKind::Path(_) => match analysis.expression(expression) {
                Some(TypeFact::Unknown | TypeFact::Any | TypeFact::Union(_)) | None => {
                    ContractActual::Dynamic
                }
                Some(fact) => ContractActual::Exact(fact.clone()),
            },
            HirExprKind::Binary {
                op:
                    Some(
                        HirBinaryOp::Equal
                        | HirBinaryOp::NotEqual
                        | HirBinaryOp::IdentityEqual
                        | HirBinaryOp::IdentityNotEqual
                        | HirBinaryOp::Less
                        | HirBinaryOp::LessEqual
                        | HirBinaryOp::Greater
                        | HirBinaryOp::GreaterEqual
                        | HirBinaryOp::And
                        | HirBinaryOp::Or,
                    ),
                ..
            } => ContractActual::Exact(TypeFact::BOOL),
            _ => ContractActual::Dynamic,
        })
    }

    fn type_fact_for_contract(&self, contract: &MirTypeContract) -> CompileResult<TypeFact> {
        Ok(match contract {
            MirTypeContract::Any => TypeFact::Any,
            MirTypeContract::Primitive(primitive) => TypeFact::primitive(*primitive),
            MirTypeContract::Range => TypeFact::Range,
            MirTypeContract::Array(element) => TypeFact::array(
                element
                    .as_deref()
                    .map(|contract| self.type_fact_for_contract(contract))
                    .transpose()?
                    .unwrap_or(TypeFact::Unknown),
            ),
            MirTypeContract::Map { key, value } => TypeFact::map(
                key.as_deref()
                    .map(|contract| self.type_fact_for_contract(contract))
                    .transpose()?
                    .unwrap_or(TypeFact::Unknown),
                value
                    .as_deref()
                    .map(|contract| self.type_fact_for_contract(contract))
                    .transpose()?
                    .unwrap_or(TypeFact::Unknown),
            ),
            MirTypeContract::Set(element) => TypeFact::set(
                element
                    .as_deref()
                    .map(|contract| self.type_fact_for_contract(contract))
                    .transpose()?
                    .unwrap_or(TypeFact::Unknown),
            ),
            MirTypeContract::Iterator(item) => TypeFact::iterator(
                item.as_deref()
                    .map(|contract| self.type_fact_for_contract(contract))
                    .transpose()?
                    .unwrap_or(TypeFact::Unknown),
            ),
            MirTypeContract::Tuple(elements) => TypeFact::tuple(
                elements
                    .iter()
                    .map(|element| {
                        element
                            .as_ref()
                            .map(|contract| self.type_fact_for_contract(contract))
                            .transpose()
                            .map(|fact| fact.unwrap_or(TypeFact::Unknown))
                    })
                    .collect::<CompileResult<Vec<_>>>()?,
            ),
            MirTypeContract::Option(some) => TypeFact::option(
                some.as_deref()
                    .map(|contract| self.type_fact_for_contract(contract))
                    .transpose()?
                    .unwrap_or(TypeFact::Unknown),
            ),
            MirTypeContract::Result { ok, err } => TypeFact::result(
                ok.as_deref()
                    .map(|contract| self.type_fact_for_contract(contract))
                    .transpose()?
                    .unwrap_or(TypeFact::Unknown),
                err.as_deref()
                    .map(|contract| self.type_fact_for_contract(contract))
                    .transpose()?
                    .unwrap_or(TypeFact::Unknown),
            ),
            MirTypeContract::Callable {
                kind: MirCallableKind::Function,
                positional_arity,
            } => TypeFact::function(
                positional_arity
                    .map(|arity| vec![TypeFact::Unknown; arity as usize])
                    .unwrap_or_default(),
                TypeFact::Unknown,
            ),
            MirTypeContract::Callable {
                kind: MirCallableKind::Closure,
                ..
            } => TypeFact::Closure,
            MirTypeContract::Definition(type_id) | MirTypeContract::Shape { type_id, .. } => {
                let name = self.type_name(*type_id).ok_or_else(registry_input_error)?;
                match self.catalog.ty(*type_id).map(|definition| definition.kind) {
                    Some(vela_registry::TypeKindDef::Host) => TypeFact::host(name),
                    Some(vela_registry::TypeKindDef::ScriptEnum) => {
                        TypeFact::enum_type(name, None::<String>)
                    }
                    _ => TypeFact::record(name),
                }
            }
            MirTypeContract::Variant {
                type_id, variant, ..
            } => {
                let name = self.type_name(*type_id).ok_or_else(registry_input_error)?;
                let variant = self
                    .catalog
                    .variant(*variant)
                    .map(|variant| variant.path.name.clone())
                    .or_else(|| {
                        self.variant_ids
                            .iter()
                            .find_map(|((_, name), id)| (*id == *variant).then(|| name.clone()))
                    })
                    .ok_or_else(registry_input_error)?;
                TypeFact::enum_type(name, Some(variant))
            }
            MirTypeContract::Host(target) => TypeFact::host(
                self.type_name(target.semantic)
                    .ok_or_else(registry_input_error)?,
            ),
        })
    }

    fn type_name(&self, type_id: vela_def::TypeId) -> Option<String> {
        self.type_names.get(&type_id).cloned().or_else(|| {
            self.catalog
                .ty(type_id)
                .map(|definition| super::external::source_name(&definition.path))
        })
    }
}

fn constant_actual(value: &crate::Constant) -> ContractActual {
    match value {
        crate::Constant::Unit => ContractActual::Exact(TypeFact::UNIT),
        crate::Constant::Bool(_) => ContractActual::Exact(TypeFact::BOOL),
        crate::Constant::Char(_) => ContractActual::Exact(TypeFact::CHAR),
        crate::Constant::Scalar(value) => {
            ContractActual::Exact(TypeFact::primitive(value.primitive_tag()))
        }
        crate::Constant::String(_) => ContractActual::Exact(TypeFact::STRING),
        crate::Constant::Bytes(_) => ContractActual::Exact(TypeFact::BYTES),
        crate::Constant::Array(_) | crate::Constant::Map(_) => ContractActual::Dynamic,
    }
}

fn numeric_kind(
    graph: &vela_hir::module_graph::ModuleGraph,
    expression: HirExprId,
) -> Option<NumericLiteralKind> {
    let record = graph
        .bodies()
        .find_map(|body| body.expression(expression))?;
    match &record.kind {
        HirExprKind::Literal(HirLiteral::Integer(value)) if value.suffix.is_none() => {
            Some(NumericLiteralKind::Integer)
        }
        HirExprKind::Literal(HirLiteral::Float(value)) if value.suffix.is_none() => {
            Some(NumericLiteralKind::Float)
        }
        HirExprKind::Unary {
            op: Some(HirUnaryOp::Negate),
            operand: Some(operand),
        } => numeric_kind(graph, *operand),
        _ => None,
    }
}
