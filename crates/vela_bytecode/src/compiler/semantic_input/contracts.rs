use std::collections::BTreeMap;

use vela_analysis::contracts::{
    ContractActual, ExpectedCallableContract, ExpectedCallableKind, ExpectedContractContext,
    ExpectedContractOutcome, check_expected_callable_contract, check_expected_callable_contract_at,
    check_expected_contract, check_expected_contract_at,
};
use vela_analysis::literals::{
    LiteralPrimitiveContext, NumericLiteralKind, NumericLiteralUse,
    supports_deferred_numeric_literal,
};
use vela_analysis::semantic_facts::OperatorTargetFact;
use vela_analysis::type_fact::TypeFact;
use vela_common::{PrimitiveTag, Span};
use vela_def::FunctionId;
use vela_hir::body::{HirBinaryOp, HirExprKind};
use vela_hir::ids::HirExprId;
use vela_mir::{
    CompileGuardKey, CompileGuardTarget, MirBuildError, MirGuardContext, MirGuardLocation,
    MirTypeContract,
};

use super::{GenerationBuilder, input_error, registry_input_error};
use crate::compiler::error::CompileResult;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ContractBoundary {
    function: FunctionId,
    expression: HirExprId,
    expected: MirTypeContract,
    context: ExpectedContractContext,
    guard_context: MirGuardContext,
}

impl ContractBoundary {
    pub(super) fn function_parameter(
        function: FunctionId,
        expression: HirExprId,
        expected: MirTypeContract,
        name: String,
        index: u32,
    ) -> Self {
        Self {
            function,
            expression,
            expected,
            context: ExpectedContractContext::FunctionParameter { name: name.clone() },
            guard_context: MirGuardContext::new(MirGuardLocation::Parameter { index }, name),
        }
    }

    pub(super) fn native_parameter(
        function: FunctionId,
        expression: HirExprId,
        expected: MirTypeContract,
        display_function: impl Into<String>,
        name: impl Into<String>,
        index: u32,
    ) -> Self {
        let display_function = display_function.into();
        let name = name.into();
        Self {
            function,
            expression,
            expected,
            context: ExpectedContractContext::NativeParameter {
                function: display_function,
                name: name.clone(),
                index,
            },
            guard_context: MirGuardContext::new(MirGuardLocation::Parameter { index }, name),
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
            context: ExpectedContractContext::TypedLet { name: name.clone() },
            guard_context: MirGuardContext::new(MirGuardLocation::Local, name),
        }
    }

    pub(super) fn field(
        function: FunctionId,
        expression: HirExprId,
        expected: MirTypeContract,
        name: impl Into<String>,
    ) -> Self {
        let name = name.into();
        Self {
            function,
            expression,
            expected,
            context: ExpectedContractContext::Field { name: name.clone() },
            guard_context: MirGuardContext::new(MirGuardLocation::Field, name),
        }
    }

    fn has_native_parameter_site(
        &self,
        function: FunctionId,
        expression: HirExprId,
        context: &ExpectedContractContext,
    ) -> bool {
        self.function == function && self.expression == expression && &self.context == context
    }
}

impl GenerationBuilder<'_, '_> {
    pub(super) fn replace_native_parameter_boundary(
        &mut self,
        function: FunctionId,
        expression: HirExprId,
        expected: MirTypeContract,
        display_function: impl Into<String>,
        name: impl Into<String>,
        index: u32,
    ) {
        let replacement = ContractBoundary::native_parameter(
            function,
            expression,
            expected,
            display_function,
            name,
            index,
        );
        self.boundaries.retain(|boundary| {
            !boundary.has_native_parameter_site(function, expression, &replacement.context)
        });
        self.boundaries.push(replacement);
    }

    pub(super) fn remove_native_parameter_boundary(
        &mut self,
        function: FunctionId,
        expression: HirExprId,
        display_function: impl Into<String>,
        name: impl Into<String>,
        index: u32,
    ) {
        let site = ContractBoundary::native_parameter(
            function,
            expression,
            MirTypeContract::Any,
            display_function,
            name,
            index,
        );
        self.boundaries.retain(|boundary| {
            !boundary.has_native_parameter_site(function, expression, &site.context)
        });
    }

    pub(super) fn reject_literal_diagnostics(&self) -> CompileResult<()> {
        let diagnostics = self.literal_diagnostics()?;
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(crate::compiler::error::CompileError::new(
                crate::compiler::error::CompileErrorKind::SemanticDiagnostics(diagnostics),
            ))
        }
    }

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
                accepted_kinds,
                positional_arity,
            } => check_expected_callable_contract(
                actual,
                ExpectedCallableContract::new(
                    if accepted_kinds.accepts_direct_function() {
                        ExpectedCallableKind::Function
                    } else {
                        ExpectedCallableKind::Closure
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
        for diagnostic in self.literal_diagnostics()? {
            if !self.diagnostics.contains(&diagnostic) {
                self.diagnostics.push(diagnostic);
            }
        }

        let mut checked = BTreeMap::<
            (FunctionId, HirExprId),
            (MirTypeContract, ExpectedContractContext, MirGuardContext),
        >::new();
        for boundary in self.boundaries.clone() {
            let key = (boundary.function, boundary.expression);
            if let Some((expected, context, guard_context)) = checked.get(&key) {
                if expected == &boundary.expected
                    && context == &boundary.context
                    && guard_context == &boundary.guard_context
                {
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
            checked.insert(
                key,
                (
                    boundary.expected.clone(),
                    boundary.context.clone(),
                    boundary.guard_context.clone(),
                ),
            );
            let actual = self.contract_actual(boundary.function, boundary.expression)?;
            let validation = match &boundary.expected {
                MirTypeContract::Callable {
                    accepted_kinds,
                    positional_arity,
                } => check_expected_callable_contract_at(
                    boundary.expression,
                    actual,
                    ExpectedCallableContract::new(
                        if accepted_kinds.accepts_direct_function() {
                            ExpectedCallableKind::Function
                        } else {
                            ExpectedCallableKind::Closure
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
                                    context: boundary.guard_context,
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

    fn literal_diagnostics(&self) -> CompileResult<Vec<vela_common::Diagnostic>> {
        let mut diagnostics = Vec::new();
        for (function, _) in self.selected_executable_roots()? {
            for diagnostic in self
                .executable_analysis(function)?
                .literal_diagnostics(self.request.graph)
            {
                if !diagnostics.contains(&diagnostic) {
                    diagnostics.push(diagnostic);
                }
            }
        }
        Ok(diagnostics)
    }

    pub(super) fn literal_contexts(
        &self,
    ) -> CompileResult<BTreeMap<FunctionId, BTreeMap<HirExprId, LiteralPrimitiveContext>>> {
        let mut contexts =
            BTreeMap::<FunctionId, BTreeMap<HirExprId, LiteralPrimitiveContext>>::new();
        for (function, root) in self.selected_executable_roots()? {
            self.collect_binary_literal_contexts(
                function,
                &self.executable_body_ids(root),
                contexts.entry(function).or_default(),
            )?;
        }
        for boundary in &self.boundaries {
            let Some(literal) = self
                .body_for_expression(boundary.expression)
                .and_then(|body| NumericLiteralUse::classify(body, boundary.expression))
                .filter(|literal| literal.supports_direct_contract_context())
            else {
                continue;
            };
            let MirTypeContract::Primitive(primitive) = boundary.expected else {
                continue;
            };
            let context = LiteralPrimitiveContext::Expected(primitive);
            let function_contexts = contexts.entry(boundary.function).or_default();
            if let Some(previous) =
                function_contexts.insert(literal.resolution_expression(), context)
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

    fn collect_binary_literal_contexts(
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
                    op: Some(operation),
                    lhs: Some(lhs),
                    rhs: Some(rhs),
                } = expression.kind
                else {
                    continue;
                };
                if !supports_deferred_numeric_literal(operation) {
                    continue;
                }
                let lhs_literal = NumericLiteralUse::classify(body, lhs);
                let rhs_literal = NumericLiteralUse::classify(body, rhs);
                let (peer, literal) = match (lhs_literal, rhs_literal) {
                    (None, Some(literal)) => (lhs, literal),
                    (Some(literal), None) => (rhs, literal),
                    (Some(_), Some(_)) | (None, None) => continue,
                };
                let operator = analysis.operator_target(expression.id);
                let context = match (operator, analysis.expression(peer)) {
                    (Some(OperatorTargetFact::Binary(_)), Some(TypeFact::Primitive(primitive)))
                        if primitive.numeric_tag().is_some()
                            && matches!(literal.kind(), NumericLiteralKind::Float)
                                == matches!(primitive, PrimitiveTag::F32 | PrimitiveTag::F64) =>
                    {
                        if operation == HirBinaryOp::Add
                            && matches!(literal.kind(), NumericLiteralKind::Float)
                            && literal.supports_deferred_operation(operation)
                        {
                            LiteralPrimitiveContext::DeferredDynamic
                        } else {
                            LiteralPrimitiveContext::Expected(*primitive)
                        }
                    }
                    (Some(OperatorTargetFact::Dynamic), _)
                        if literal.supports_deferred_operation(operation) =>
                    {
                        LiteralPrimitiveContext::DeferredDynamic
                    }
                    _ => continue,
                };
                contexts.insert(literal.resolution_expression(), context);
            }
        }
        Ok(())
    }

    fn contract_actual(
        &self,
        function: FunctionId,
        expression: HirExprId,
    ) -> CompileResult<ContractActual> {
        if let Some(literal) = self
            .body_for_expression(expression)
            .and_then(|body| NumericLiteralUse::classify(body, expression))
            .filter(|literal| literal.supports_direct_contract_context())
        {
            return Ok(ContractActual::DeferredNumeric(literal.kind()));
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
            HirExprKind::Literal(_) => analyzed_contract_actual(analysis.expression(expression)),
            HirExprKind::Unit => ContractActual::Exact(TypeFact::UNIT),
            HirExprKind::Lambda { .. } => ContractActual::Exact(TypeFact::Closure),
            HirExprKind::Path(_) => analyzed_contract_actual(analysis.expression(expression)),
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
            HirExprKind::Tuple { .. } | HirExprKind::Array { .. } | HirExprKind::Map { .. } => {
                analyzed_contract_actual(analysis.expression(expression))
            }
            _ => ContractActual::Dynamic,
        })
    }

    fn type_fact_for_contract(&self, contract: &MirTypeContract) -> CompileResult<TypeFact> {
        Ok(match contract {
            MirTypeContract::Any => TypeFact::Any,
            MirTypeContract::TaskError => TypeFact::record("task::Error"),
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
                accepted_kinds,
                positional_arity,
            } if accepted_kinds.accepts_direct_function() => TypeFact::function(
                positional_arity
                    .map(|arity| vec![TypeFact::Unknown; arity as usize])
                    .unwrap_or_default(),
                TypeFact::Unknown,
            ),
            MirTypeContract::Callable { .. } => TypeFact::Closure,
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

pub(super) fn typed_container_mutation_arg_fact(
    receiver: Option<&TypeFact>,
    method: &str,
    param_name: &str,
    position: usize,
) -> Option<TypeFact> {
    let receiver = project_mutation_contract_fact(receiver?)?;
    match receiver {
        TypeFact::Array { element }
        | TypeFact::ArrayMut {
            element,
            mutation: vela_common::CollectionViewMutation::Growable,
        } => match (method, mutation_arg_role(method, param_name, position)) {
            ("push" | "insert", MutationArgRole::Value) => Some(*element),
            ("extend", MutationArgRole::Values) => Some(TypeFact::Array { element }),
            _ => None,
        },
        TypeFact::Map { key, value }
        | TypeFact::MapMut {
            key,
            value,
            mutation: vela_common::CollectionViewMutation::Growable,
        } => match (method, mutation_arg_role(method, param_name, position)) {
            ("set", MutationArgRole::Key)
                if !matches!(key.as_ref(), TypeFact::Primitive(PrimitiveTag::String)) =>
            {
                Some(*key)
            }
            ("set", MutationArgRole::Value) => Some(*value),
            ("extend", MutationArgRole::Values) => Some(TypeFact::Map { key, value }),
            _ => None,
        },
        TypeFact::Set { element }
        | TypeFact::SetMut {
            element,
            mutation: vela_common::CollectionViewMutation::Growable,
        } => match (method, mutation_arg_role(method, param_name, position)) {
            ("add", MutationArgRole::Value) => Some(*element),
            ("extend", MutationArgRole::Values) => Some(TypeFact::Set { element }),
            _ => None,
        },
        TypeFact::Unknown
        | TypeFact::Never
        | TypeFact::Any
        | TypeFact::Primitive(_)
        | TypeFact::Range
        | TypeFact::ArrayView { .. }
        | TypeFact::ArrayMut { .. }
        | TypeFact::MapView { .. }
        | TypeFact::MapMut { .. }
        | TypeFact::SetView { .. }
        | TypeFact::SetMut { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::ScopedIterator { .. }
        | TypeFact::Tuple { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Function { .. }
        | TypeFact::Closure
        | TypeFact::Record { .. }
        | TypeFact::LogicalRecord(_)
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Trait { .. }
        | TypeFact::Module { .. }
        | TypeFact::Union(_) => None,
    }
}

fn project_mutation_contract_fact(fact: &TypeFact) -> Option<TypeFact> {
    Some(match fact {
        TypeFact::Primitive(primitive) => TypeFact::Primitive(*primitive),
        TypeFact::Range => TypeFact::Range,
        TypeFact::Array { element } => TypeFact::array(project_mutation_contract_fact(element)?),
        TypeFact::ArrayView { element } => {
            TypeFact::array_view(project_mutation_contract_fact(element)?)
        }
        TypeFact::ArrayMut { element, mutation } => {
            TypeFact::array_mut(project_mutation_contract_fact(element)?, *mutation)
        }
        TypeFact::Map { key, value } => TypeFact::map(
            project_mutation_contract_fact(key)?,
            project_mutation_contract_fact(value)?,
        ),
        TypeFact::MapView { key, value } => TypeFact::map_view(
            project_mutation_contract_fact(key)?,
            project_mutation_contract_fact(value)?,
        ),
        TypeFact::MapMut {
            key,
            value,
            mutation,
        } => TypeFact::map_mut(
            project_mutation_contract_fact(key)?,
            project_mutation_contract_fact(value)?,
            *mutation,
        ),
        TypeFact::Set { element } => TypeFact::set(project_mutation_contract_fact(element)?),
        TypeFact::SetView { element } => {
            TypeFact::set_view(project_mutation_contract_fact(element)?)
        }
        TypeFact::SetMut { element, mutation } => {
            TypeFact::set_mut(project_mutation_contract_fact(element)?, *mutation)
        }
        TypeFact::Iterator { item } => TypeFact::iterator(project_mutation_contract_fact(item)?),
        TypeFact::ScopedIterator { item } => {
            TypeFact::scoped_iterator(project_mutation_contract_fact(item)?)
        }
        TypeFact::Tuple { elements } => TypeFact::tuple(
            elements
                .iter()
                .map(project_mutation_contract_fact)
                .collect::<Option<Vec<_>>>()?,
        ),
        TypeFact::Option { some } | TypeFact::OptionSome { some } => {
            TypeFact::option(project_mutation_contract_fact(some)?)
        }
        TypeFact::OptionNone => TypeFact::option(TypeFact::Unknown),
        TypeFact::Result { ok, err } => TypeFact::result(
            project_mutation_contract_fact(ok)?,
            project_mutation_contract_fact(err)?,
        ),
        TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. } => {
            TypeFact::result(TypeFact::Unknown, TypeFact::Unknown)
        }
        TypeFact::Function { .. } => TypeFact::function(Vec::new(), TypeFact::Unknown),
        TypeFact::Closure => TypeFact::Closure,
        TypeFact::Unknown
        | TypeFact::Never
        | TypeFact::Any
        | TypeFact::Record { .. }
        | TypeFact::LogicalRecord(_)
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Trait { .. }
        | TypeFact::Module { .. }
        | TypeFact::Union(_) => return None,
    })
}

pub(super) fn mutation_arg_debug_name(method: &str, param_name: &str, position: usize) -> String {
    if param_name.is_empty() {
        match mutation_arg_role(method, param_name, position) {
            MutationArgRole::Key => "key",
            MutationArgRole::Value => "value",
            MutationArgRole::Values => "values",
            MutationArgRole::Other => "argument",
        }
        .to_owned()
    } else {
        param_name.to_owned()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MutationArgRole {
    Key,
    Value,
    Values,
    Other,
}

fn mutation_arg_role(method: &str, param_name: &str, position: usize) -> MutationArgRole {
    match param_name {
        "key" => MutationArgRole::Key,
        "value" => MutationArgRole::Value,
        "values" => MutationArgRole::Values,
        _ => match (method, position) {
            ("set", 0) => MutationArgRole::Key,
            ("set", 1) | ("insert", 1) | ("push", 0) | ("add", 0) => MutationArgRole::Value,
            ("extend", 0) => MutationArgRole::Values,
            _ => MutationArgRole::Other,
        },
    }
}

fn constant_actual(value: &vela_mir::MirEvaluatedConstant) -> ContractActual {
    match value {
        vela_mir::MirEvaluatedConstant::Unit => ContractActual::Exact(TypeFact::UNIT),
        vela_mir::MirEvaluatedConstant::Bool(_) => ContractActual::Exact(TypeFact::BOOL),
        vela_mir::MirEvaluatedConstant::Char(_) => ContractActual::Exact(TypeFact::CHAR),
        vela_mir::MirEvaluatedConstant::Scalar(value) => {
            ContractActual::Exact(TypeFact::primitive(value.primitive_tag()))
        }
        vela_mir::MirEvaluatedConstant::String(_) => ContractActual::Exact(TypeFact::STRING),
        vela_mir::MirEvaluatedConstant::Bytes(_) => ContractActual::Exact(TypeFact::BYTES),
        vela_mir::MirEvaluatedConstant::Array(_) | vela_mir::MirEvaluatedConstant::Map(_) => {
            ContractActual::Dynamic
        }
    }
}

fn analyzed_contract_actual(fact: Option<&TypeFact>) -> ContractActual {
    match fact {
        Some(TypeFact::Unknown | TypeFact::Any | TypeFact::Union(_)) | None => {
            ContractActual::Dynamic
        }
        Some(fact) => ContractActual::Exact(fact.clone()),
    }
}
