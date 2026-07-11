use std::collections::BTreeMap;

use vela_analysis::type_fact::TypeFact;
use vela_hir::binding::BindingResolution;
use vela_hir::body::{HirBody, HirBodyRoot, HirExprKind, HirPatternKind, HirStmtKind};
use vela_hir::ids::{HirBlockId, HirExprId, HirLocalId, HirStmtId};

use crate::{
    DebugLocalKind, MirBuildError, MirDebugLocal, MirEffect, MirFunction, MirFunctionOwner,
    MirFunctionReturn, MirImmediate, MirLiveRegion, MirLocalId, MirLoweringInput, MirOperand,
    MirParameterKind, MirParameterSpec, MirPlace, MirRvalue, MirSourceOrigin, MirStatement,
    MirTerminator, MirTerminatorKind, MirValueType,
};

pub(super) struct FunctionBuilder<'a> {
    pub(super) input: MirLoweringInput<'a>,
    pub(super) body: &'a HirBody,
    pub(super) function: MirFunction,
    pub(super) current_block: crate::MirBlockId,
    locals: BTreeMap<HirLocalId, MirLocalId>,
    pub(super) loop_stack: Vec<super::loops::LoopContext>,
}

impl<'a> FunctionBuilder<'a> {
    pub(super) fn new(
        input: MirLoweringInput<'a>,
        owner: MirFunctionOwner,
    ) -> Result<Self, MirBuildError> {
        let body =
            input
                .graph()
                .body(input.body())
                .ok_or(MirBuildError::MissingCompilationRoot {
                    function: input.function(),
                    body: input.body(),
                })?;
        let origin = MirSourceOrigin::body(body.id, body.origin.span);
        let descriptor = input
            .targets()
            .function_descriptor(input.function())
            .ok_or_else(|| MirBuildError::InconsistentInput {
                origin,
                message: format!(
                    "MIR lowering input lost function descriptor #{}",
                    input.function().get()
                ),
            })?;
        let return_contract = descriptor
            .signature
            .return_contract
            .clone()
            .map(|contract| MirFunctionReturn { contract, origin });
        let function = MirFunction::new(
            body.id,
            owner,
            descriptor.canonical_symbol.clone(),
            return_contract,
            origin,
        );
        let current_block = function.entry_block();
        Ok(Self {
            input,
            body,
            function,
            current_block,
            locals: BTreeMap::new(),
            loop_stack: Vec::new(),
        })
    }

    pub(super) fn build(mut self) -> Result<MirFunction, MirBuildError> {
        if self.input.config().compute_liveness {
            return Err(self.unsupported(self.body_origin(), "MIR liveness computation"));
        }
        if self.function.return_contract().is_some() {
            return Err(self.unsupported(self.body_origin(), "function return contract guard"));
        }
        self.install_locals()?;
        match self.body.root {
            HirBodyRoot::Block(block) => {
                self.lower_block(block)?;
                self.finish_open_block(None, self.body_origin())?;
            }
            HirBodyRoot::Expr(expression) => {
                let value = self.lower_expression(expression)?;
                self.finish_open_block(Some(value), self.expression_origin(expression)?)?;
            }
            HirBodyRoot::Empty => {
                self.finish_open_block(None, self.body_origin())?;
            }
        }
        Ok(self.function)
    }

    fn install_locals(&mut self) -> Result<(), MirBuildError> {
        let bindings = self
            .input
            .graph()
            .bindings_for_body(self.body.id)
            .ok_or_else(|| self.inconsistent(self.body_origin(), "HIR body has no binding map"))?;
        let descriptor = self
            .input
            .targets()
            .function_descriptor(self.input.function())
            .ok_or_else(|| {
                self.inconsistent(self.body_origin(), "function descriptor disappeared")
            })?;
        if descriptor.signature.parameters.len() != self.body.params.len() {
            return Err(self.inconsistent(
                self.body_origin(),
                format!(
                    "function signature has {} parameters but HIR body has {}",
                    descriptor.signature.parameters.len(),
                    self.body.params.len()
                ),
            ));
        }

        for (parameter_index, (parameter, target)) in self
            .body
            .params
            .iter()
            .zip(&descriptor.signature.parameters)
            .enumerate()
        {
            let binding = bindings.local(parameter.local).ok_or_else(|| {
                self.inconsistent(
                    MirSourceOrigin::body(self.body.id, parameter.origin.span),
                    format!("parameter {:?} has no local binding", parameter.id),
                )
            })?;
            if binding.name != target.name {
                return Err(self.inconsistent(
                    MirSourceOrigin::body(self.body.id, parameter.origin.span),
                    format!(
                        "parameter {} is named {:?} in HIR but {:?} in compile targets",
                        parameter_index, binding.name, target.name
                    ),
                ));
            }
            if target.contract.is_some() {
                return Err(self.unsupported(
                    MirSourceOrigin::body(self.body.id, parameter.origin.span),
                    "function parameter contract guard",
                ));
            }
            let default_body = match target.default {
                crate::CompileParameterDefault::Required => {
                    if parameter.default_body.is_some() {
                        return Err(self.inconsistent(
                            MirSourceOrigin::body(self.body.id, parameter.origin.span),
                            "HIR parameter default is missing from compile targets",
                        ));
                    }
                    None
                }
                crate::CompileParameterDefault::HirBody(default) => {
                    if parameter.default_body != Some(default) {
                        return Err(self.inconsistent(
                            MirSourceOrigin::body(self.body.id, parameter.origin.span),
                            "parameter default body disagrees with compile targets",
                        ));
                    }
                    return Err(self.unsupported(
                        MirSourceOrigin::body(self.body.id, parameter.origin.span),
                        "parameter default prologue",
                    ));
                }
                crate::CompileParameterDefault::RuntimeProvided => {
                    return Err(self.inconsistent(
                        MirSourceOrigin::body(self.body.id, parameter.origin.span),
                        "script function parameter cannot have a runtime-provided default",
                    ));
                }
            };
            let origin = MirSourceOrigin::body(self.body.id, parameter.origin.span);
            let kind = if matches!(
                self.input.identity(),
                crate::CompileFunctionIdentity::Method(_)
            ) && self.body.self_binding == Some(parameter.local)
            {
                MirParameterKind::Receiver
            } else {
                MirParameterKind::Explicit(parameter.id)
            };
            let storage = self.function.add_parameter(MirParameterSpec {
                hir_local: parameter.local,
                kind,
                name: binding.name.clone(),
                value_type: value_type(self.input.analysis().local(parameter.local)),
                contract: target.contract.clone(),
                default_body,
                origin,
            });
            self.locals.insert(parameter.local, storage);
        }

        for hir_local in &self.body.locals {
            if self.locals.contains_key(hir_local) {
                continue;
            }
            let binding = bindings.local(*hir_local).ok_or_else(|| {
                self.inconsistent(
                    self.body_origin(),
                    format!("HIR local {hir_local:?} has no binding record"),
                )
            })?;
            let origin = MirSourceOrigin::body(self.body.id, binding.span);
            let storage = self.function.add_script_local(
                *hir_local,
                value_type(self.input.analysis().local(*hir_local)),
                origin,
            );
            self.locals.insert(*hir_local, storage);
        }

        if self.input.config().emit_debug_locals {
            for hir_local in &self.body.locals {
                let binding = bindings.local(*hir_local).ok_or_else(|| {
                    self.inconsistent(
                        self.body_origin(),
                        format!("HIR local {hir_local:?} has no binding record"),
                    )
                })?;
                let storage = self.local(*hir_local, self.body_origin())?;
                let scope = self
                    .body
                    .scopes
                    .values()
                    .find(|scope| scope.locals.contains(hir_local))
                    .map(|scope| scope.id)
                    .ok_or_else(|| {
                        self.inconsistent(
                            self.body_origin(),
                            format!("HIR local {hir_local:?} has no owning scope"),
                        )
                    })?;
                self.function.add_debug_local(MirDebugLocal {
                    storage,
                    name: binding.name.clone(),
                    kind: DebugLocalKind::from(binding.kind),
                    hir_local: Some(*hir_local),
                    scope,
                    origin: MirSourceOrigin::body(self.body.id, binding.span),
                    live_region: MirLiveRegion::default(),
                });
            }
        }
        Ok(())
    }

    pub(super) fn lower_block(&mut self, block: HirBlockId) -> Result<(), MirBuildError> {
        let block = self.body.blocks.get(&block).ok_or_else(|| {
            self.inconsistent(self.body_origin(), format!("missing HIR block {block:?}"))
        })?;
        let statements = block.statements.clone();
        for statement_id in statements {
            if self.current_is_terminated()? {
                break;
            }
            self.lower_statement(statement_id)?;
        }
        Ok(())
    }

    pub(super) fn lower_statement(&mut self, statement_id: HirStmtId) -> Result<(), MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(());
        }
        let statement = self
            .body
            .statements
            .get(&statement_id)
            .ok_or_else(|| {
                self.inconsistent(
                    self.body_origin(),
                    format!("missing HIR statement {statement_id:?}"),
                )
            })?
            .clone();
        let origin = MirSourceOrigin::statement(self.body.id, statement.id, statement.origin.span);
        match statement.kind {
            HirStmtKind::Let {
                pattern,
                initializer,
                ..
            } => self.lower_let(pattern, initializer, origin),
            HirStmtKind::Return { value } => {
                let value = value
                    .map(|value| self.lower_expression(value))
                    .transpose()?;
                if self.current_is_terminated()? {
                    return Ok(());
                }
                self.function.set_terminator(
                    self.current_block,
                    MirTerminator::new(
                        origin,
                        MirTerminatorKind::Return(value),
                        MirEffect::PURE,
                        None,
                    ),
                )
            }
            HirStmtKind::Block(block) => self.lower_block(block),
            HirStmtKind::Expr {
                expression: Some(expression),
                ..
            } => self.lower_expression(expression).map(|_| ()),
            HirStmtKind::Expr {
                expression: None, ..
            } => Ok(()),
            HirStmtKind::Break => self.lower_break(statement_id, origin),
            HirStmtKind::Continue => self.lower_continue(statement_id, origin),
            HirStmtKind::For {
                patterns,
                iterable,
                body,
            } => self.lower_for(statement_id, &patterns, iterable, body, origin),
            HirStmtKind::If(value) => self.lower_if_statement(&value, origin),
            HirStmtKind::Match(_) => Err(self.unsupported(origin, "match statement")),
        }
    }

    pub(super) fn lower_let(
        &mut self,
        pattern: Option<vela_hir::ids::HirPatternId>,
        initializer: Option<HirExprId>,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        let value = initializer
            .map(|expression| self.lower_expression(expression))
            .transpose()?
            .unwrap_or(MirOperand::Immediate(MirImmediate::Unit));
        if self.current_is_terminated()? {
            return Ok(());
        }
        let Some(pattern) = pattern else {
            return Ok(());
        };
        let pattern =
            self.body.patterns.get(&pattern).ok_or_else(|| {
                self.inconsistent(origin, format!("missing HIR pattern {pattern:?}"))
            })?;
        let HirPatternKind::Binding { local: Some(local) } = pattern.kind else {
            return Err(self.unsupported(origin, "destructuring let pattern"));
        };
        let storage = self.local(local, origin)?;
        self.function.append_statement(
            self.current_block,
            MirStatement::assign(origin, MirPlace::local(storage), MirRvalue::Use(value)),
        )?;
        Ok(())
    }

    pub(super) fn lower_expression(
        &mut self,
        expression: HirExprId,
    ) -> Result<MirOperand, MirBuildError> {
        let record = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(
                self.body_origin(),
                format!("missing HIR expression {expression:?}"),
            )
        })?;
        let kind = record.kind.clone();
        let origin = MirSourceOrigin::expression(self.body.id, expression, record.origin.span);
        if let Some(value) = self.lower_aggregate_expression(expression, origin)? {
            return Ok(value);
        }
        match kind {
            HirExprKind::Literal(literal) => self.lower_literal(expression, &literal, origin),
            HirExprKind::Path(_) => self.lower_path(expression, origin),
            HirExprKind::Paren {
                expression: Some(inner),
            } => self.lower_expression(inner),
            HirExprKind::Unit => Ok(MirOperand::Immediate(MirImmediate::Unit)),
            HirExprKind::Missing => {
                Err(self.inconsistent(origin, "missing expression reached MIR lowering"))
            }
            HirExprKind::Paren { expression: None } => {
                Err(self.inconsistent(origin, "empty parenthesized expression reached MIR"))
            }
            HirExprKind::Tuple { .. } => Err(self.unsupported(origin, "tuple expression")),
            HirExprKind::Unary { op, operand } => self.lower_unary(expression, op, operand, origin),
            HirExprKind::Binary { op, lhs, rhs } => {
                self.lower_binary(expression, op, lhs, rhs, origin)
            }
            HirExprKind::Assign { op, target, value } => {
                self.lower_assignment(expression, op, target, value, origin)
            }
            HirExprKind::Field(field) => self.lower_field(expression, &field, origin),
            HirExprKind::Call(call) => self.lower_call(expression, &call, origin),
            HirExprKind::Index(index) => self.lower_index(expression, &index, origin),
            HirExprKind::Try { .. } => Err(self.unsupported(origin, "try expression")),
            HirExprKind::Array { .. } => Err(self.unsupported(origin, "array expression")),
            HirExprKind::Map { .. } => Err(self.unsupported(origin, "map expression")),
            HirExprKind::Record { .. } => Err(self.unsupported(origin, "record expression")),
            HirExprKind::Lambda { .. } => Err(self.unsupported(origin, "lambda expression")),
            HirExprKind::Block { block } => self.lower_block_expression(expression, block, origin),
            HirExprKind::If(value) => self.lower_if_expression(expression, &value, origin),
            HirExprKind::Match(_) => Err(self.unsupported(origin, "match expression")),
        }
    }

    fn lower_path(
        &self,
        expression: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let bindings = self
            .input
            .graph()
            .bindings_for_body(self.body.id)
            .ok_or_else(|| self.inconsistent(origin, "HIR body has no binding map"))?;
        match bindings.resolution(expression) {
            Some(BindingResolution::Local(local)) => {
                Ok(MirOperand::Local(self.local(*local, origin)?))
            }
            Some(BindingResolution::Declaration(_)) => {
                Err(self.unsupported(origin, "declaration value path"))
            }
            Some(BindingResolution::Import(_) | BindingResolution::QualifiedPath(_)) => {
                Err(self.inconsistent(origin, "unresolved import or qualified path reached MIR"))
            }
            None => Err(self.inconsistent(origin, "value path has no binding resolution")),
        }
    }

    fn finish_open_block(
        &mut self,
        value: Option<MirOperand>,
        origin: MirSourceOrigin,
    ) -> Result<(), MirBuildError> {
        if !self.current_is_terminated()? {
            self.function.set_terminator(
                self.current_block,
                MirTerminator::new(
                    origin,
                    MirTerminatorKind::Return(value),
                    MirEffect::PURE,
                    None,
                ),
            )?;
        }
        Ok(())
    }

    pub(super) fn current_is_terminated(&self) -> Result<bool, MirBuildError> {
        self.function
            .block(self.current_block)
            .map(|block| block.terminator().is_some())
            .ok_or(MirBuildError::MissingBlock {
                block: self.current_block,
                origin: self.body_origin(),
            })
    }

    pub(super) fn local(
        &self,
        local: HirLocalId,
        origin: MirSourceOrigin,
    ) -> Result<MirLocalId, MirBuildError> {
        self.locals
            .get(&local)
            .copied()
            .ok_or_else(|| self.inconsistent(origin, format!("missing MIR storage for {local:?}")))
    }

    /// Stabilize a value that must survive lowering a later source operand.
    ///
    /// Mutable script locals are reads at the eventual use site, so keeping a
    /// `MirOperand::Local` across later lowering could observe an intervening
    /// assignment. Immediates and already-defined temporaries are stable.
    pub(super) fn capture_operand(
        &mut self,
        operand: MirOperand,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        if self.current_is_terminated()? {
            return Ok(MirOperand::Immediate(MirImmediate::Unit));
        }
        let MirOperand::Local(local) = operand else {
            return Ok(operand);
        };
        let value_type = self
            .function
            .local(local)
            .map(|local| local.value_type)
            .ok_or(MirBuildError::MissingLocal { local, origin })?;
        let temp = self.function.add_temp(value_type, origin);
        self.function.append_statement(
            self.current_block,
            MirStatement::assign(
                origin,
                MirPlace::temp(temp),
                MirRvalue::Use(MirOperand::Local(local)),
            ),
        )?;
        Ok(MirOperand::Temp(temp))
    }

    fn body_origin(&self) -> MirSourceOrigin {
        MirSourceOrigin::body(self.body.id, self.body.origin.span)
    }

    fn expression_origin(&self, expression: HirExprId) -> Result<MirSourceOrigin, MirBuildError> {
        let record = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(
                self.body_origin(),
                format!("missing HIR expression {expression:?}"),
            )
        })?;
        Ok(MirSourceOrigin::expression(
            self.body.id,
            expression,
            record.origin.span,
        ))
    }

    pub(super) fn inconsistent(
        &self,
        origin: MirSourceOrigin,
        message: impl Into<String>,
    ) -> MirBuildError {
        MirBuildError::InconsistentInput {
            origin,
            message: message.into(),
        }
    }

    pub(super) fn unsupported(&self, origin: MirSourceOrigin, feature: &str) -> MirBuildError {
        self.inconsistent(
            origin,
            format!("{feature} is outside the current MIR builder slice"),
        )
    }
}

pub(super) fn value_type(fact: Option<&TypeFact>) -> MirValueType {
    match fact {
        Some(TypeFact::Primitive(vela_common::PrimitiveTag::Unit)) => MirValueType::Unit,
        Some(TypeFact::Primitive(primitive)) => MirValueType::Primitive(*primitive),
        Some(TypeFact::Range) => MirValueType::Range,
        Some(TypeFact::Iterator { .. }) => MirValueType::Iterator,
        Some(TypeFact::Tuple { elements }) => {
            MirValueType::Tuple(u32::try_from(elements.len()).unwrap_or(u32::MAX))
        }
        Some(TypeFact::Function { .. } | TypeFact::Closure) => MirValueType::Callable,
        Some(
            TypeFact::Unknown
            | TypeFact::Never
            | TypeFact::Any
            | TypeFact::Array { .. }
            | TypeFact::Map { .. }
            | TypeFact::Set { .. }
            | TypeFact::Option { .. }
            | TypeFact::OptionSome { .. }
            | TypeFact::OptionNone
            | TypeFact::Result { .. }
            | TypeFact::ResultOk { .. }
            | TypeFact::ResultErr { .. }
            | TypeFact::Record { .. }
            | TypeFact::LogicalRecord(_)
            | TypeFact::Enum { .. }
            | TypeFact::Host { .. }
            | TypeFact::Trait { .. }
            | TypeFact::Module { .. }
            | TypeFact::Union(_),
        )
        | None => MirValueType::Dynamic,
    }
}
