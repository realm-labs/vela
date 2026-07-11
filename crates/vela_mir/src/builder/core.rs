use std::collections::BTreeMap;

use vela_analysis::type_fact::TypeFact;
use vela_hir::binding::BindingResolution;
use vela_hir::body::{HirBody, HirExprKind, HirStmtKind};
use vela_hir::ids::{HirBlockId, HirBodyId, HirExprId, HirLocalId, HirStmtId};

use crate::{
    CompileLambdaTarget, CompileParameter, DebugLocalKind, MirBuildError, MirDebugLocal, MirEffect,
    MirFunction, MirFunctionId, MirFunctionOwner, MirFunctionReturn, MirImmediate, MirLiveRegion,
    MirLocalId, MirLoweringInput, MirOperand, MirParameterKind, MirParameterSpec, MirPlace,
    MirRvalue, MirSourceOrigin, MirStatement, MirTerminator, MirTerminatorKind, MirValueType,
};

#[derive(Clone, Debug)]
enum BuilderFunctionKind {
    Root { parameters: Vec<CompileParameter> },
    Lambda { target: CompileLambdaTarget },
}

pub(super) struct FunctionBuilder<'a> {
    pub(super) input: MirLoweringInput<'a>,
    pub(super) body: &'a HirBody,
    pub(super) function: MirFunction,
    pub(super) current_block: crate::MirBlockId,
    locals: BTreeMap<HirLocalId, MirLocalId>,
    nested_functions: BTreeMap<HirBodyId, MirFunctionId>,
    kind: BuilderFunctionKind,
    pub(super) loop_stack: Vec<super::loops::LoopContext>,
}

impl<'a> FunctionBuilder<'a> {
    pub(super) fn new_root(
        input: MirLoweringInput<'a>,
        owner: MirFunctionOwner,
        nested_functions: BTreeMap<HirBodyId, MirFunctionId>,
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
            nested_functions,
            kind: BuilderFunctionKind::Root {
                parameters: descriptor.signature.parameters.clone(),
            },
            loop_stack: Vec::new(),
        })
    }

    pub(super) fn new_lambda(
        input: MirLoweringInput<'a>,
        owner: MirFunctionOwner,
        target: &CompileLambdaTarget,
        nested_functions: BTreeMap<HirBodyId, MirFunctionId>,
    ) -> Result<Self, MirBuildError> {
        let body = input
            .graph()
            .body(target.body)
            .ok_or(MirBuildError::MissingHirBody {
                body: target.body,
                origin: target.origin,
            })?;
        let origin = MirSourceOrigin::body(body.id, body.origin.span);
        if target.origin != origin {
            return Err(MirBuildError::InconsistentInput {
                origin,
                message: "lambda compile target origin disagrees with Heavy HIR".to_owned(),
            });
        }
        let function = MirFunction::new(body.id, owner, target.code_symbol.clone(), None, origin);
        let current_block = function.entry_block();
        Ok(Self {
            input,
            body,
            function,
            current_block,
            locals: BTreeMap::new(),
            nested_functions,
            kind: BuilderFunctionKind::Lambda {
                target: target.clone(),
            },
            loop_stack: Vec::new(),
        })
    }

    pub(super) fn build(mut self) -> Result<MirFunction, MirBuildError> {
        if self.input.config().compute_liveness {
            return Err(self.unsupported(self.body_origin(), "MIR liveness computation"));
        }
        self.install_locals()?;
        self.lower_parameter_defaults()?;
        self.lower_owning_body()?;
        Ok(self.function)
    }

    fn install_locals(&mut self) -> Result<(), MirBuildError> {
        let graph = self.input.graph();
        let bindings = graph
            .bindings_for_body(self.body.id)
            .ok_or_else(|| self.inconsistent(self.body_origin(), "HIR body has no binding map"))?;
        self.install_captures(bindings)?;
        self.install_parameters(bindings)?;

        let default_bodies = self.parameter_default_bodies()?;
        for body in &default_bodies {
            self.install_body_locals(body, bindings)?;
        }
        self.install_body_locals(self.body, bindings)?;

        if self.input.config().emit_debug_locals {
            self.install_capture_debug_locals(bindings)?;
            let mut emitted = self
                .body
                .captures
                .iter()
                .map(|capture| capture.local)
                .collect::<std::collections::BTreeSet<_>>();
            for parameter in &self.body.params {
                self.install_debug_local(self.body, parameter.local, bindings, None)?;
                emitted.insert(parameter.local);
            }
            for body in &default_bodies {
                for local in &body.locals {
                    if emitted.insert(*local) {
                        self.install_debug_local(body, *local, bindings, None)?;
                    }
                }
            }
            for local in &self.body.locals {
                if emitted.insert(*local) {
                    self.install_debug_local(self.body, *local, bindings, None)?;
                }
            }
        }
        Ok(())
    }

    fn install_captures(
        &mut self,
        bindings: &vela_hir::binding::BindingMap,
    ) -> Result<(), MirBuildError> {
        if matches!(self.kind, BuilderFunctionKind::Root { .. }) {
            if !self.body.captures.is_empty() {
                return Err(self.inconsistent(
                    self.body_origin(),
                    "compilation-root HIR body unexpectedly owns lambda captures",
                ));
            }
            return Ok(());
        }
        for capture in self.body.captures.clone() {
            if capture.owner != self.body.id {
                return Err(self.inconsistent(
                    self.body_origin(),
                    format!("capture {:?} belongs to a different HIR body", capture.id),
                ));
            }
            let binding = bindings.local(capture.local).ok_or_else(|| {
                self.inconsistent(
                    self.body_origin(),
                    format!("capture {:?} has no source-local binding", capture.id),
                )
            })?;
            let (use_body, use_expression) = self
                .input
                .graph()
                .bodies()
                .find_map(|body| {
                    body.expression(capture.use_expression)
                        .map(|expression| (body, expression))
                })
                .ok_or_else(|| {
                    self.inconsistent(
                        self.body_origin(),
                        format!(
                            "capture {:?} refers to missing use expression {:?}",
                            capture.id, capture.use_expression
                        ),
                    )
                })?;
            if !self
                .input
                .graph()
                .body_and_ancestors(use_body.id)
                .any(|body| body.id == self.body.id)
            {
                return Err(self.inconsistent(
                    self.body_origin(),
                    format!(
                        "capture {:?} use expression is outside its lambda subtree",
                        capture.id
                    ),
                ));
            }
            if !matches!(
                bindings.resolution(capture.use_expression),
                Some(BindingResolution::Local(local)) if *local == capture.local
            ) {
                return Err(self.inconsistent(
                    MirSourceOrigin::expression(
                        use_body.id,
                        capture.use_expression,
                        use_expression.origin.span,
                    ),
                    format!(
                        "capture {:?} disagrees with its binding resolution",
                        capture.id
                    ),
                ));
            }
            let origin = MirSourceOrigin::expression(
                use_body.id,
                capture.use_expression,
                use_expression.origin.span,
            );
            let storage = self.function.add_capture(
                capture.id,
                capture.local,
                binding.name.clone(),
                value_type(self.input.analysis().local(capture.local)),
                origin,
            );
            if self.locals.insert(capture.local, storage).is_some() {
                return Err(self.inconsistent(
                    origin,
                    format!("lambda repeats capture local {:?}", capture.local),
                ));
            }
        }
        Ok(())
    }

    fn install_parameters(
        &mut self,
        bindings: &vela_hir::binding::BindingMap,
    ) -> Result<(), MirBuildError> {
        let targets = match &self.kind {
            BuilderFunctionKind::Root { parameters } => parameters.clone(),
            BuilderFunctionKind::Lambda { target } => {
                if target.parameters.len() != self.body.params.len() {
                    return Err(self.inconsistent(
                        self.body_origin(),
                        "lambda compile-target parameter count disagrees with Heavy HIR",
                    ));
                }
                target
                    .parameters
                    .iter()
                    .zip(&self.body.params)
                    .map(|(target, parameter)| {
                        if target.parameter != parameter.id
                            || target.local != parameter.local
                            || target.origin
                                != MirSourceOrigin::body(self.body.id, parameter.origin.span)
                        {
                            return Err(self.inconsistent(
                                target.origin,
                                "lambda parameter target disagrees with Heavy HIR order or identity",
                            ));
                        }
                        if parameter.default_body.is_some() {
                            return Err(self.inconsistent(
                                target.origin,
                                "lambda parameter unexpectedly owns a default body",
                            ));
                        }
                        Ok(CompileParameter {
                            name: target.name.clone(),
                            contract: target.contract.clone(),
                            default: crate::CompileParameterDefault::Required,
                            origin: Some(target.origin),
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
        };
        if targets.len() != self.body.params.len() {
            return Err(self.inconsistent(
                self.body_origin(),
                format!(
                    "function signature has {} parameters but HIR body has {}",
                    targets.len(),
                    self.body.params.len()
                ),
            ));
        }

        for (parameter_index, (parameter, target)) in
            self.body.params.iter().zip(&targets).enumerate()
        {
            let binding = bindings.local(parameter.local).ok_or_else(|| {
                self.inconsistent(
                    MirSourceOrigin::body(self.body.id, parameter.origin.span),
                    format!("parameter {:?} has no local binding", parameter.id),
                )
            })?;
            let origin = MirSourceOrigin::body(self.body.id, parameter.origin.span);
            if binding.name != target.name {
                return Err(self.inconsistent(
                    origin,
                    format!(
                        "parameter {} is named {:?} in HIR but {:?} in compile targets",
                        parameter_index, binding.name, target.name
                    ),
                ));
            }
            let default_body = match target.default {
                crate::CompileParameterDefault::Required => {
                    if parameter.default_body.is_some() {
                        return Err(self.inconsistent(
                            origin,
                            "HIR parameter default is missing from compile targets",
                        ));
                    }
                    None
                }
                crate::CompileParameterDefault::HirBody(default) => {
                    if parameter.default_body != Some(default) {
                        return Err(self.inconsistent(
                            origin,
                            "parameter default body disagrees with compile targets",
                        ));
                    }
                    Some(default)
                }
                crate::CompileParameterDefault::RuntimeProvided => {
                    return Err(self.inconsistent(
                        origin,
                        "script function parameter cannot have a runtime-provided default",
                    ));
                }
            };
            let kind = if matches!(self.function.owner(), MirFunctionOwner::Method(_))
                && self.body.self_binding == Some(parameter.local)
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
            if self.locals.insert(parameter.local, storage).is_some() {
                return Err(self.inconsistent(
                    origin,
                    format!("parameter repeats local {:?}", parameter.local),
                ));
            }
        }
        Ok(())
    }

    fn parameter_default_bodies(&self) -> Result<Vec<&'a HirBody>, MirBuildError> {
        let mut bodies = Vec::new();
        for parameter in self.function.parameters() {
            let Some(body) = parameter.default_body else {
                continue;
            };
            let body = self
                .input
                .graph()
                .body(body)
                .ok_or(MirBuildError::MissingHirBody {
                    body,
                    origin: parameter.origin,
                })?;
            bodies.push(body);
        }
        Ok(bodies)
    }

    fn install_body_locals(
        &mut self,
        body: &HirBody,
        bindings: &vela_hir::binding::BindingMap,
    ) -> Result<(), MirBuildError> {
        for hir_local in &body.locals {
            if self.locals.contains_key(hir_local) {
                continue;
            }
            let binding = bindings.local(*hir_local).ok_or_else(|| {
                self.inconsistent(
                    MirSourceOrigin::body(body.id, body.origin.span),
                    format!("HIR local {hir_local:?} has no binding record"),
                )
            })?;
            let origin = MirSourceOrigin::body(body.id, binding.span);
            let storage = self.function.add_script_local(
                *hir_local,
                value_type(self.input.analysis().local(*hir_local)),
                origin,
            );
            self.locals.insert(*hir_local, storage);
        }
        Ok(())
    }

    fn install_capture_debug_locals(
        &mut self,
        bindings: &vela_hir::binding::BindingMap,
    ) -> Result<(), MirBuildError> {
        for capture in self.function.captures().to_vec() {
            let binding = bindings.local(capture.source_local).ok_or_else(|| {
                self.inconsistent(self.body_origin(), "capture has no binding record")
            })?;
            let storage = self.local(capture.source_local, capture.origin)?;
            self.function.add_debug_local(MirDebugLocal {
                storage,
                name: binding.name.clone(),
                kind: DebugLocalKind::Capture,
                hir_local: Some(capture.source_local),
                scope: self.body.root_scope,
                origin: capture.origin,
                live_region: MirLiveRegion::default(),
            });
        }
        Ok(())
    }

    fn install_debug_local(
        &mut self,
        body: &HirBody,
        hir_local: HirLocalId,
        bindings: &vela_hir::binding::BindingMap,
        kind: Option<DebugLocalKind>,
    ) -> Result<(), MirBuildError> {
        let origin = MirSourceOrigin::body(body.id, body.origin.span);
        let binding = bindings.local(hir_local).ok_or_else(|| {
            self.inconsistent(
                origin,
                format!("HIR local {hir_local:?} has no binding record"),
            )
        })?;
        let storage = self.local(hir_local, origin)?;
        let scope = body
            .scopes
            .values()
            .find(|scope| scope.locals.contains(&hir_local))
            .map(|scope| scope.id)
            .ok_or_else(|| {
                self.inconsistent(
                    origin,
                    format!("HIR local {hir_local:?} has no owning scope"),
                )
            })?;
        self.function.add_debug_local(MirDebugLocal {
            storage,
            name: binding.name.clone(),
            kind: kind.unwrap_or_else(|| DebugLocalKind::from(binding.kind)),
            hir_local: Some(hir_local),
            scope,
            origin: MirSourceOrigin::body(body.id, binding.span),
            live_region: MirLiveRegion::default(),
        });
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
            HirStmtKind::Match(value) => self.lower_match_statement(&value, origin),
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
        self.lower_let_pattern(pattern, value, origin)
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
        let value = if let Some(value) = self.lower_aggregate_expression(expression, origin)? {
            value
        } else {
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
                HirExprKind::Unary { op, operand } => {
                    self.lower_unary(expression, op, operand, origin)
                }
                HirExprKind::Binary { op, lhs, rhs } => {
                    self.lower_binary(expression, op, lhs, rhs, origin)
                }
                HirExprKind::Assign { op, target, value } => {
                    self.lower_assignment(expression, op, target, value, origin)
                }
                HirExprKind::Field(field) => self.lower_field(expression, &field, origin),
                HirExprKind::Call(call) => {
                    if self.input.targets().constructor(expression).is_some() {
                        self.lower_constructor(expression, origin)
                    } else {
                        self.lower_call(expression, &call, origin)
                    }
                }
                HirExprKind::Index(index) => self.lower_index(expression, &index, origin),
                HirExprKind::Try {
                    expression: operand,
                } => self.lower_try_expression(expression, operand, origin),
                HirExprKind::Array { .. } => Err(self.unsupported(origin, "array expression")),
                HirExprKind::Map { .. } => Err(self.unsupported(origin, "map expression")),
                HirExprKind::Record { .. } => self.lower_constructor(expression, origin),
                HirExprKind::Lambda { body } => self.lower_lambda(expression, body, origin),
                HirExprKind::Block { block } => {
                    self.lower_block_expression(expression, block, origin)
                }
                HirExprKind::If(value) => self.lower_if_expression(expression, &value, origin),
                HirExprKind::Match(value) => {
                    self.lower_match_expression(expression, &value, origin)
                }
            }?
        };
        self.apply_expression_guard(expression, value, origin)
    }

    fn lower_path(
        &mut self,
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
            Some(BindingResolution::Declaration(declaration)) => {
                self.lower_declaration_path(expression, *declaration, origin)
            }
            Some(BindingResolution::Import(_) | BindingResolution::QualifiedPath(_)) => {
                Err(self.inconsistent(origin, "unresolved import or qualified path reached MIR"))
            }
            None => Err(self.inconsistent(origin, "value path has no binding resolution")),
        }
    }

    pub(super) fn finish_open_block(
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

    pub(super) fn body_origin(&self) -> MirSourceOrigin {
        MirSourceOrigin::body(self.body.id, self.body.origin.span)
    }

    pub(super) fn expression_origin(
        &self,
        expression: HirExprId,
    ) -> Result<MirSourceOrigin, MirBuildError> {
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

    pub(super) fn nested_function(
        &self,
        body: HirBodyId,
        origin: MirSourceOrigin,
    ) -> Result<MirFunctionId, MirBuildError> {
        self.nested_functions.get(&body).copied().ok_or_else(|| {
            self.inconsistent(
                origin,
                format!("missing generation-local MIR function for lambda body {body:?}"),
            )
        })
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
