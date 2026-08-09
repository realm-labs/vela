use std::collections::{BTreeMap, BTreeSet};

use vela_host::target::HostTargetPlan;
use vela_mir::{
    CompileTryFamily, CompileTryTarget, DebugLocalKind, MirAggregate, MirBackendHandoff,
    MirBinaryOp, MirBlockId, MirCall, MirContextualBinaryOp, MirDynamicUnaryOp, MirFieldTarget,
    MirFormatPart, MirFunction, MirFunctionAnalyses, MirFunctionId, MirGuardAssumption,
    MirHostOperation, MirHostPath, MirHostPathSegment, MirIdentityOp, MirImmediate, MirIndexKey,
    MirIndexOperation, MirIteratorOperation, MirLiteralSide, MirOperand, MirPatternPredicate,
    MirPlace, MirProgram, MirReflectionOperation, MirRvalue, MirScriptParameterGuardMode,
    MirStateOperation, MirStatementId, MirStatementKind, MirSwitchValue, MirTaskOperation,
    MirTerminatorKind, MirUnaryOp,
};

use crate::{
    BinaryLiteralOp, BinaryLiteralSide, CacheSiteId, CallArgument, Constant, DynamicCallArgument,
    FormatStringPart, FrameSlotInfo, FrameSlotKind, FunctionIndex, GuardKind, GuardLocation,
    InstructionOffset, Register, ScriptCallMode, TryPropagateFamily, UnlinkedCodeObject,
    UnlinkedInstruction, UnlinkedInstructionKind, UnlinkedParameterTypeGuard,
    UnlinkedTaskContinuation, UnlinkedTaskInstruction,
};

use crate::compiler::cache_sites::{attach_cache_site, cache_site_kind};
use crate::compiler::constant_encoding::encode_evaluated_constant;

mod operations;
mod physical;
mod scalar;
mod support;

use super::selection::{SelectedFunctionPlan, SelectedProgramPlan, SelectionError, mir_successors};
use support::{dynamic_binary_instruction, guard_kind, guard_location, mir_reaches, type_guard};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MirBackendError {
    InvalidSelection(SelectionError),
    MissingRoot,
    MissingMirFunction(MirFunctionId),
    MissingBlock(MirBlockId),
    MissingStatement,
    MissingDestination,
    MissingTarget(&'static str),
    RegisterOverflow,
    DynamicHostArgumentOverflow,
}

pub(crate) fn compile(
    handoff: MirBackendHandoff<'_>,
    plan: &SelectedProgramPlan,
) -> Result<UnlinkedCodeObject, MirBackendError> {
    let program = handoff.program();
    let analyses = handoff.all_analyses();
    let (root_id, root) = program
        .functions()
        .next()
        .ok_or(MirBackendError::MissingRoot)?;
    compile_function(program, analyses, plan, root_id, root)
}

fn compile_function(
    program: &MirProgram,
    analyses: &BTreeMap<MirFunctionId, MirFunctionAnalyses>,
    plan: &SelectedProgramPlan,
    function_id: MirFunctionId,
    function: &MirFunction,
) -> Result<UnlinkedCodeObject, MirBackendError> {
    let function_plan = plan
        .function(function_id)
        .ok_or(MirBackendError::InvalidSelection(
            SelectionError::MissingFunctionPlan(function_id),
        ))?;
    let mut backend = FunctionBackend::new(
        program,
        analyses,
        plan,
        function_plan,
        function_id,
        function,
    )?;
    backend.compile()?;
    Ok(backend.finish())
}

struct FunctionBackend<'a> {
    program: &'a MirProgram,
    function_id: MirFunctionId,
    function: &'a MirFunction,
    analyses: &'a BTreeMap<MirFunctionId, MirFunctionAnalyses>,
    selection: &'a SelectedProgramPlan,
    function_selection: &'a SelectedFunctionPlan,
    facts: &'a vela_mir::MirProgramPointFacts,
    budget: &'a vela_mir::MirBudgetSchedule,
    code: UnlinkedCodeObject,
    locals: BTreeMap<vela_mir::MirLocalId, Register>,
    temps: BTreeMap<vela_mir::MirTempId, Register>,
    blocks: BTreeMap<MirBlockId, InstructionOffset>,
    patches: Vec<(usize, MirBlockId, MirBlockId)>,
    next_register: u16,
    nested: BTreeMap<MirFunctionId, FunctionIndex>,
    loop_blocks: BTreeSet<MirBlockId>,
    try_join_blocks: BTreeSet<MirBlockId>,
    current_block: Option<MirBlockId>,
    current_statement: Option<MirStatementId>,
    current_terminator: Option<MirBlockId>,
    pending_execution_units: u32,
    pending_budget_charges: Vec<crate::MirBudgetCharge>,
    unspanned_spans: Vec<vela_common::Span>,
    pending_selected_units: Vec<PendingSelectedUnit>,
    pending_scalar_blocks: Vec<PendingScalarBlock>,
}

struct PendingSelectedUnit {
    instruction: InstructionOffset,
    source_points: [vela_common::Span; 2],
    budget_units: [u32; 2],
    mir_statement: MirStatementId,
    mir_terminator: MirBlockId,
}

struct PendingScalarBlock {
    instruction: InstructionOffset,
    block: MirBlockId,
    statements: Box<[MirStatementId]>,
    operations: Box<[crate::ScalarOp]>,
    exit: PendingScalarExit,
    exit_source: crate::ScalarSourcePointId,
    exit_execution_units: u32,
    source_points: Vec<vela_common::Span>,
    mir_budget_sites: Vec<crate::scalar_plan::ScalarMirBudgetSite>,
    range_loop_header: Option<MirBlockId>,
}

enum PendingScalarExit {
    Jump(MirBlockId),
    BoolBranch {
        condition: Register,
        passed: MirBlockId,
        failed: MirBlockId,
    },
}

impl<'a> FunctionBackend<'a> {
    fn new(
        program: &'a MirProgram,
        analyses: &'a BTreeMap<MirFunctionId, MirFunctionAnalyses>,
        selection: &'a SelectedProgramPlan,
        function_selection: &'a SelectedFunctionPlan,
        function_id: MirFunctionId,
        function: &'a MirFunction,
    ) -> Result<Self, MirBackendError> {
        let mut next_register = 0_u16;
        let mut locals = BTreeMap::new();
        for (local, _) in function.locals() {
            locals.insert(local, Register(next_register));
            next_register = next_register
                .checked_add(1)
                .ok_or(MirBackendError::RegisterOverflow)?;
        }
        let mut temps = BTreeMap::new();
        for (temp, _) in function.temps() {
            temps.insert(temp, Register(next_register));
            next_register = next_register
                .checked_add(1)
                .ok_or(MirBackendError::RegisterOverflow)?;
        }

        let params = function
            .parameters()
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<Vec<_>>();
        let defaults = function
            .parameters()
            .iter()
            .map(|parameter| parameter.default_body.is_some())
            .collect::<Vec<_>>();
        let capture_count = u16::try_from(function.captures().len())
            .map_err(|_| MirBackendError::RegisterOverflow)?;
        let mut code = UnlinkedCodeObject::new(function.code_symbol(), next_register)
            .with_asyncness(function.asyncness())
            .with_params(params)
            .with_param_defaults(defaults)
            .with_capture_count(capture_count);

        for debug in function.debug_locals().map(|(_, debug)| debug) {
            let kind = match debug.kind {
                DebugLocalKind::Parameter
                    if matches!(function.owner(), vela_mir::MirFunctionOwner::Lambda { .. }) =>
                {
                    FrameSlotKind::LambdaParameter
                }
                DebugLocalKind::Parameter => FrameSlotKind::Parameter,
                DebugLocalKind::Local | DebugLocalKind::Synthetic => FrameSlotKind::Local,
                DebugLocalKind::LoopBinding => FrameSlotKind::ForBinding,
                DebugLocalKind::PatternBinding => FrameSlotKind::PatternBinding,
                DebugLocalKind::Capture => FrameSlotKind::Capture,
            };
            code.frame.push_slot(FrameSlotInfo::new(
                debug.name.clone(),
                locals[&debug.storage],
                kind,
                debug.hir_local,
                Some(debug.origin.span),
            ));
        }

        for (index, parameter) in function.parameters().iter().enumerate() {
            if let Some(contract) = &parameter.contract {
                let guard = type_guard(
                    program,
                    contract,
                    GuardKind::Contract,
                    GuardLocation::Parameter {
                        index: u16::try_from(index)
                            .map_err(|_| MirBackendError::RegisterOverflow)?,
                    },
                    &parameter.name,
                )?;
                code.param_guards.push(UnlinkedParameterTypeGuard {
                    parameter: u16::try_from(index)
                        .map_err(|_| MirBackendError::RegisterOverflow)?,
                    guard,
                });
            }
        }
        if let Some(return_contract) = function.return_contract() {
            code.return_guard = Some(type_guard(
                program,
                &return_contract.contract,
                GuardKind::Contract,
                GuardLocation::Return,
                "return",
            )?);
        }

        let block_ids = function.blocks().map(|(id, _)| id).collect::<Vec<_>>();
        let mut loop_blocks = BTreeSet::new();
        let mut try_join_blocks = BTreeSet::new();
        let mut unspanned_spans = Vec::new();
        for (source, block) in function.blocks() {
            let Some(terminator) = block.terminator() else {
                continue;
            };
            for target in mir_successors(&terminator.kind) {
                if target <= source && mir_reaches(function, target, source) {
                    loop_blocks.extend(
                        block_ids
                            .iter()
                            .copied()
                            .filter(|block| *block >= target && *block <= source),
                    );
                }
            }
            if let MirTerminatorKind::TrySwitch { join, .. } = terminator.kind {
                try_join_blocks.insert(join);
            }
            if matches!(
                terminator.kind,
                MirTerminatorKind::RangeNext {
                    mode: vela_mir::MirRangeStepMode::I64Proven,
                    ..
                }
            ) && !unspanned_spans.contains(&terminator.origin.span)
            {
                unspanned_spans.push(terminator.origin.span);
            }
        }
        Ok(Self {
            program,
            analyses,
            selection,
            function_selection,
            facts: &analyses
                .get(&function_id)
                .ok_or(MirBackendError::MissingMirFunction(function_id))?
                .facts,
            budget: &analyses
                .get(&function_id)
                .ok_or(MirBackendError::MissingMirFunction(function_id))?
                .budget,
            function_id,
            function,
            code,
            locals,
            temps,
            blocks: BTreeMap::new(),
            patches: Vec::new(),
            next_register,
            nested: BTreeMap::new(),
            loop_blocks,
            try_join_blocks,
            current_block: None,
            current_statement: None,
            current_terminator: None,
            pending_execution_units: 0,
            pending_budget_charges: Vec::new(),
            unspanned_spans,
            pending_selected_units: Vec::new(),
            pending_scalar_blocks: Vec::new(),
        })
    }

    fn compile(&mut self) -> Result<(), MirBackendError> {
        self.compile_nested_functions()?;
        let blocks = self.function.blocks().collect::<Vec<_>>();
        for (index, (block_id, block)) in blocks.iter().copied().enumerate() {
            self.current_block = Some(block_id);
            let next = blocks.get(index + 1).map(|(id, _)| *id);
            self.blocks
                .insert(block_id, InstructionOffset(self.code.instructions.len()));
            let scalar_block = self.function_selection.scalar_block(block_id).cloned();
            let superinstruction = self.function_selection.superinstruction(block_id).cloned();
            if let Some(selected) = &scalar_block {
                let terminator = block
                    .terminator()
                    .ok_or(MirBackendError::MissingBlock(block_id))?;
                self.current_terminator = Some(block_id);
                self.scalar_block(selected, &terminator.kind, terminator.origin.span)?;
                self.current_terminator = None;
                continue;
            }
            for statement_id in block.statements() {
                let statement = self
                    .function
                    .statement(*statement_id)
                    .ok_or(MirBackendError::MissingStatement)?;
                self.current_statement = Some(*statement_id);
                if let Some(point) = self.budget.statement_before(*statement_id) {
                    self.emit_execution_units(
                        vela_mir::MirBudgetSite::StatementBefore(*statement_id),
                        point,
                    );
                }
                if superinstruction
                    .as_ref()
                    .is_none_or(|selected| selected.statement() != *statement_id)
                {
                    self.statement(statement)?;
                }
            }
            self.current_statement = None;
            let terminator = block
                .terminator()
                .ok_or(MirBackendError::MissingBlock(block_id))?;
            self.current_terminator = Some(block_id);
            if let Some(point) = self.budget.terminator_before(block_id) {
                self.emit_execution_units(
                    vela_mir::MirBudgetSite::TerminatorBefore(block_id),
                    point,
                );
            }
            if let Some(selected) = &superinstruction {
                self.superinstruction(selected, terminator.origin.span, next)?;
            } else {
                self.terminator(&terminator.kind, terminator.origin.span, next)?;
            }
            self.current_terminator = None;
        }
        self.patch_targets()?;
        self.finalize_scalar_blocks()?;
        self.finalize_selected_units()?;
        self.attach_cache_sites();
        self.code.register_count = self.next_register;
        Ok(())
    }

    fn compile_nested_functions(&mut self) -> Result<(), MirBackendError> {
        let nested = self
            .program
            .functions()
            .filter(|(_, function)| matches!(function.owner(), vela_mir::MirFunctionOwner::Lambda { parent, .. } if *parent == self.function_id))
            .collect::<Vec<_>>();
        for (id, function) in nested {
            let code = compile_function(self.program, self.analyses, self.selection, id, function)?;
            let index = self.code.push_nested_function(code);
            self.nested.insert(id, index);
        }
        Ok(())
    }

    fn finish(self) -> UnlinkedCodeObject {
        self.code
    }

    fn statement(&mut self, statement: &vela_mir::MirStatement) -> Result<(), MirBackendError> {
        let dst = statement.destination.map(|place| self.place(place));
        let span = statement.origin.span;
        match &statement.kind {
            MirStatementKind::Assign(value) => {
                self.assign(dst.ok_or(MirBackendError::MissingDestination)?, value, span)?
            }
            MirStatementKind::Unary { operation, operand } => {
                let src = self.operand(operand, span)?;
                let kind = match operation {
                    MirUnaryOp::NotBool => UnlinkedInstructionKind::Not {
                        dst: dst.ok_or(MirBackendError::MissingDestination)?,
                        src,
                    },
                    MirUnaryOp::Negate(_) => UnlinkedInstructionKind::Negate {
                        dst: dst.ok_or(MirBackendError::MissingDestination)?,
                        src,
                    },
                };
                self.emit(kind, span);
            }
            MirStatementKind::Binary {
                operation,
                left,
                right,
            } => {
                let lhs = self.operand(left, span)?;
                let rhs = self.operand(right, span)?;
                let dst = dst.ok_or(MirBackendError::MissingDestination)?;
                let kind = self.select_binary(*operation, dst, lhs, rhs, right);
                self.emit(kind, span);
            }
            MirStatementKind::DynamicUnary { operation, operand } => {
                let src = self.operand(operand, span)?;
                let dst = dst.ok_or(MirBackendError::MissingDestination)?;
                self.emit(
                    match operation {
                        MirDynamicUnaryOp::Negate => UnlinkedInstructionKind::Negate { dst, src },
                        MirDynamicUnaryOp::Not => UnlinkedInstructionKind::Not { dst, src },
                    },
                    span,
                );
            }
            MirStatementKind::DynamicBinary {
                operation,
                left,
                right,
            } => {
                let lhs = self.operand(left, span)?;
                let rhs = self.operand(right, span)?;
                let dst = dst.ok_or(MirBackendError::MissingDestination)?;
                self.emit(dynamic_binary_instruction(*operation, dst, lhs, rhs), span);
            }
            MirStatementKind::ContextualNumericBinary {
                operation,
                value,
                literal,
                literal_side,
            } => {
                let value = self.operand(value, span)?;
                let dst = dst.ok_or(MirBackendError::MissingDestination)?;
                let op = match operation {
                    MirContextualBinaryOp::Add => BinaryLiteralOp::Add,
                    MirContextualBinaryOp::Subtract => BinaryLiteralOp::Sub,
                    MirContextualBinaryOp::Multiply => BinaryLiteralOp::Mul,
                    MirContextualBinaryOp::Divide => BinaryLiteralOp::Div,
                    MirContextualBinaryOp::Remainder => BinaryLiteralOp::Rem,
                    MirContextualBinaryOp::Less => BinaryLiteralOp::Less,
                    MirContextualBinaryOp::LessEqual => BinaryLiteralOp::LessEqual,
                    MirContextualBinaryOp::Greater => BinaryLiteralOp::Greater,
                    MirContextualBinaryOp::GreaterEqual => BinaryLiteralOp::GreaterEqual,
                };
                let side = match literal_side {
                    MirLiteralSide::Left => BinaryLiteralSide::Left,
                    MirLiteralSide::Right => BinaryLiteralSide::Right,
                };
                let text = literal.text().to_owned();
                self.emit(
                    if literal.is_float() {
                        UnlinkedInstructionKind::BinaryFloatLiteral {
                            dst,
                            op,
                            value,
                            literal: text,
                            side,
                        }
                    } else {
                        UnlinkedInstructionKind::BinaryIntLiteral {
                            dst,
                            op,
                            value,
                            literal: text,
                            side,
                        }
                    },
                    span,
                );
            }
            MirStatementKind::IdentityCompare {
                operation,
                left,
                right,
            } => {
                let lhs = self.operand(left, span)?;
                let rhs = self.operand(right, span)?;
                let dst = dst.ok_or(MirBackendError::MissingDestination)?;
                self.emit(
                    match operation {
                        MirIdentityOp::Equal => {
                            UnlinkedInstructionKind::IdentityEqual { dst, lhs, rhs }
                        }
                        MirIdentityOp::NotEqual => {
                            UnlinkedInstructionKind::IdentityNotEqual { dst, lhs, rhs }
                        }
                    },
                    span,
                );
            }
            MirStatementKind::TupleField { tuple, index } => {
                let value = self.operand(tuple, span)?;
                self.emit(
                    UnlinkedInstructionKind::GetTupleField {
                        dst: dst.ok_or(MirBackendError::MissingDestination)?,
                        value,
                        index: *index as usize,
                    },
                    span,
                );
            }
            MirStatementKind::ReadField { receiver, target } => self.read_field(
                dst.ok_or(MirBackendError::MissingDestination)?,
                receiver,
                target,
                span,
            )?,
            MirStatementKind::WriteField {
                receiver,
                target,
                value,
            } => self.write_field(receiver, target, value, span)?,
            MirStatementKind::Index(operation) => self.index(dst, operation, span)?,
            MirStatementKind::State(operation) => {
                let state = match operation {
                    MirStateOperation::ReadVmState { state }
                    | MirStateOperation::WriteVmState { state, .. }
                    | MirStateOperation::ReadExternState { state } => *state,
                };
                let target = self
                    .program
                    .targets()
                    .state(state)
                    .ok_or(MirBackendError::MissingTarget("state"))?;
                let instruction = match operation {
                    MirStateOperation::ReadVmState { .. } => UnlinkedInstructionKind::LoadState {
                        dst: dst.ok_or(MirBackendError::MissingDestination)?,
                        state: target.name.clone(),
                        slot: self.state_slot(state),
                        cache_site: None,
                    },
                    MirStateOperation::WriteVmState { value, .. } => {
                        UnlinkedInstructionKind::StoreState {
                            state: target.name.clone(),
                            slot: self.state_slot(state),
                            src: self.operand(value, span)?,
                        }
                    }
                    MirStateOperation::ReadExternState { .. } => {
                        UnlinkedInstructionKind::LoadExternState {
                            dst: dst.ok_or(MirBackendError::MissingDestination)?,
                            state: target.name.clone(),
                            slot: self.state_slot(state),
                            cache_site: None,
                        }
                    }
                };
                self.emit(instruction, span);
            }
            MirStatementKind::Allocate(aggregate) => self.allocate(
                dst.ok_or(MirBackendError::MissingDestination)?,
                aggregate,
                span,
            )?,
            MirStatementKind::FormatString { parts } => {
                let parts = parts
                    .iter()
                    .map(|part| match part {
                        MirFormatPart::Text(text) => {
                            let id = self.code.push_constant(Constant::String(text.clone()));
                            Ok(FormatStringPart::Text(id))
                        }
                        MirFormatPart::Value(value) => {
                            Ok(FormatStringPart::Value(self.operand(value, span)?))
                        }
                    })
                    .collect::<Result<Vec<_>, MirBackendError>>()?;
                self.emit(
                    UnlinkedInstructionKind::FormatString {
                        dst: dst.ok_or(MirBackendError::MissingDestination)?,
                        parts,
                    },
                    span,
                );
            }
            MirStatementKind::MaterializeConstant(value) => {
                let constant = self.code.push_constant(encode_evaluated_constant(value));
                self.emit(
                    UnlinkedInstructionKind::LoadConst {
                        dst: dst.ok_or(MirBackendError::MissingDestination)?,
                        constant,
                    },
                    span,
                );
            }
            MirStatementKind::MakeRange {
                start,
                end,
                inclusive,
            } => {
                let start = self.operand(start, span)?;
                let end = self.operand(end, span)?;
                self.emit(
                    UnlinkedInstructionKind::MakeRange {
                        dst: dst.ok_or(MirBackendError::MissingDestination)?,
                        start,
                        end,
                        inclusive: *inclusive,
                    },
                    span,
                );
            }
            MirStatementKind::Call(call) => {
                self.call(dst.ok_or(MirBackendError::MissingDestination)?, call, span)?
            }
            MirStatementKind::Task(task) => {
                self.task(dst.ok_or(MirBackendError::MissingDestination)?, task, span)?
            }
            MirStatementKind::Host(operation) => self.host(dst, operation, span)?,
            MirStatementKind::Reflect(operation) => self.reflect(
                dst.ok_or(MirBackendError::MissingDestination)?,
                operation,
                span,
            )?,
            MirStatementKind::GuardTrap { value, guard } => {
                let value = self.operand(value, span)?;
                let guard = self
                    .function
                    .guard(*guard)
                    .ok_or(MirBackendError::MissingTarget("guard"))?;
                match &guard.assumption {
                    MirGuardAssumption::TupleArity { arity } => self.emit(
                        UnlinkedInstructionKind::GuardTupleArity {
                            value,
                            arity: *arity as usize,
                        },
                        span,
                    ),
                    MirGuardAssumption::Type(contract) => {
                        let context = guard
                            .context
                            .as_ref()
                            .ok_or(MirBackendError::MissingTarget("guard context"))?;
                        let guard = type_guard(
                            self.program,
                            contract,
                            guard_kind(context.location),
                            guard_location(context.location)?,
                            &context.debug_name,
                        )?;
                        self.emit(
                            UnlinkedInstructionKind::GuardType { src: value, guard },
                            span,
                        );
                    }
                }
            }
            MirStatementKind::Iterator(MirIteratorOperation::Create {
                iterable,
                host_collection,
            }) => {
                let iterable = self.operand(iterable, span)?;
                self.emit(
                    UnlinkedInstructionKind::IterInit {
                        dst: dst.ok_or(MirBackendError::MissingDestination)?,
                        iterable,
                        host_collection: *host_collection,
                    },
                    span,
                );
            }
        }
        Ok(())
    }

    fn emit_execution_units(
        &mut self,
        site: vela_mir::MirBudgetSite,
        point: vela_mir::MirBudgetPoint,
    ) {
        self.pending_execution_units = self
            .pending_execution_units
            .checked_add(point.units)
            .expect("verified MIR execution-unit schedule fits u32");
        self.pending_budget_charges.push(crate::MirBudgetCharge {
            site,
            class: point.class,
            units: point.units,
        });
    }

    fn assign(
        &mut self,
        dst: Register,
        value: &MirRvalue,
        span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
        match value {
            MirRvalue::Use(value) => {
                let src = self.operand(value, span)?;
                if src != dst {
                    self.emit(UnlinkedInstructionKind::Move { dst, src }, span);
                }
            }
            MirRvalue::Constant { value, .. } => self.load_immediate(dst, *value, span),
            MirRvalue::Truthy { value } => {
                let src = self.operand(value, span)?;
                self.emit(UnlinkedInstructionKind::Truthy { dst, src }, span);
            }
            MirRvalue::IsMissing { value } => {
                // The semantic predicate is consumed from its MIR definition
                // by branch lowering; no emission-order register fact is kept.
                let _ = self.operand(value, span)?;
            }
            MirRvalue::PatternPredicate(predicate) => {
                self.pattern_predicate(dst, predicate, span)?
            }
        }
        Ok(())
    }

    fn pattern_predicate(
        &mut self,
        dst: Register,
        predicate: &MirPatternPredicate,
        span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
        match predicate {
            MirPatternPredicate::TupleArity { value, arity } => {
                let value = self.operand(value, span)?;
                self.emit(
                    UnlinkedInstructionKind::TupleArityEqual {
                        dst,
                        value,
                        arity: *arity as usize,
                    },
                    span,
                );
            }
            MirPatternPredicate::NeverMatches { value } => {
                let _ = self.operand(value, span)?;
                self.load_immediate(dst, MirImmediate::Bool(false), span);
            }
            MirPatternPredicate::VariantShape {
                value,
                type_id,
                variant,
            } => {
                let value = self.operand(value, span)?;
                let ty = self
                    .program
                    .targets()
                    .type_descriptor(*type_id)
                    .ok_or(MirBackendError::MissingTarget("type"))?;
                let variant = self
                    .program
                    .targets()
                    .variant(*variant)
                    .ok_or(MirBackendError::MissingTarget("variant"))?;
                self.emit(
                    UnlinkedInstructionKind::EnumTagEqual {
                        dst,
                        value,
                        enum_name: ty.runtime_name.clone(),
                        type_id: Some(*type_id),
                        variant: variant.name.clone(),
                        variant_id: Some(variant.id),
                    },
                    span,
                );
            }
            MirPatternPredicate::DynamicVariant {
                value,
                owner_name,
                variant_name,
            } => {
                let value = self.operand(value, span)?;
                self.emit(
                    UnlinkedInstructionKind::EnumTagEqual {
                        dst,
                        value,
                        enum_name: owner_name.clone(),
                        type_id: Some(vela_def::script_type_id(
                            vela_package::PackageId::anonymous().as_str(),
                            owner_name,
                            None,
                        )),
                        variant: variant_name.clone(),
                        variant_id: Some(vela_def::script_variant_id(
                            vela_package::PackageId::anonymous().as_str(),
                            owner_name,
                            variant_name,
                            None,
                        )),
                    },
                    span,
                );
            }
        }
        Ok(())
    }

    fn allocate(
        &mut self,
        dst: Register,
        aggregate: &MirAggregate,
        span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
        let kind = match aggregate {
            MirAggregate::Tuple(values) => UnlinkedInstructionKind::MakeTuple {
                dst,
                elements: self.operands(values, span)?,
            },
            MirAggregate::Array(values) => {
                let elements = self.operands(values, span)?;
                UnlinkedInstructionKind::MakeArray { dst, elements }
            }
            MirAggregate::Map(entries) => UnlinkedInstructionKind::MakeMap {
                dst,
                entries: entries
                    .iter()
                    .map(|(key, value)| Ok((key.clone(), self.operand(value, span)?)))
                    .collect::<Result<_, MirBackendError>>()?,
            },
            MirAggregate::SetFromArray { source } => UnlinkedInstructionKind::MakeSetFromArray {
                dst,
                src: self.operand(source, span)?,
            },
            MirAggregate::Record {
                type_id, fields, ..
            } => {
                let ty = self
                    .program
                    .targets()
                    .type_descriptor(*type_id)
                    .ok_or(MirBackendError::MissingTarget("type"))?;
                let fields = fields
                    .iter()
                    .map(|(field, value)| {
                        let name = self
                            .program
                            .targets()
                            .field(*field)
                            .ok_or(MirBackendError::MissingTarget("field"))?
                            .name
                            .clone();
                        Ok((name, self.operand(value, span)?))
                    })
                    .collect::<Result<Vec<_>, MirBackendError>>()?;
                UnlinkedInstructionKind::MakeRecord {
                    dst,
                    type_name: ty.runtime_name.clone(),
                    type_id: Some(*type_id),
                    fields,
                }
            }
            MirAggregate::DynamicRecord { type_name, fields } => {
                let fields = fields
                    .iter()
                    .map(|(name, value)| Ok((name.clone(), self.operand(value, span)?)))
                    .collect::<Result<Vec<_>, MirBackendError>>()?;
                UnlinkedInstructionKind::MakeRecord {
                    dst,
                    type_name: type_name.clone(),
                    type_id: Some(vela_def::script_type_id(
                        vela_package::PackageId::anonymous().as_str(),
                        type_name,
                        None,
                    )),
                    fields,
                }
            }
            MirAggregate::Enum {
                type_id,
                variant,
                fields,
            } => {
                let ty = self
                    .program
                    .targets()
                    .type_descriptor(*type_id)
                    .ok_or(MirBackendError::MissingTarget("type"))?;
                let variant = self
                    .program
                    .targets()
                    .variant(*variant)
                    .ok_or(MirBackendError::MissingTarget("variant"))?;
                let fields = fields
                    .iter()
                    .map(|(field, value)| {
                        let name = self
                            .program
                            .targets()
                            .field(*field)
                            .ok_or(MirBackendError::MissingTarget("field"))?
                            .name
                            .clone();
                        Ok((name, self.operand(value, span)?))
                    })
                    .collect::<Result<Vec<_>, MirBackendError>>()?;
                UnlinkedInstructionKind::MakeEnum {
                    dst,
                    enum_name: ty.runtime_name.clone(),
                    type_id: Some(*type_id),
                    variant: variant.name.clone(),
                    variant_id: Some(variant.id),
                    fields,
                }
            }
            MirAggregate::DynamicVariant {
                owner_name,
                variant_name,
                fields,
            } => {
                let fields = fields
                    .iter()
                    .map(|(name, value)| Ok((name.clone(), self.operand(value, span)?)))
                    .collect::<Result<Vec<_>, MirBackendError>>()?;
                UnlinkedInstructionKind::MakeEnum {
                    dst,
                    enum_name: owner_name.clone(),
                    type_id: Some(vela_def::script_type_id(
                        vela_package::PackageId::anonymous().as_str(),
                        owner_name,
                        None,
                    )),
                    variant: variant_name.clone(),
                    variant_id: Some(vela_def::script_variant_id(
                        vela_package::PackageId::anonymous().as_str(),
                        owner_name,
                        variant_name,
                        None,
                    )),
                    fields,
                }
            }
            MirAggregate::Closure { function, captures } => {
                let index = *self
                    .nested
                    .get(function)
                    .ok_or(MirBackendError::MissingMirFunction(*function))?;
                UnlinkedInstructionKind::MakeClosure {
                    dst,
                    function: index,
                    captures: self.operands(captures, span)?,
                }
            }
        };
        self.emit(kind, span);
        Ok(())
    }

    fn read_field(
        &mut self,
        dst: Register,
        receiver: &MirOperand,
        target: &MirFieldTarget,
        span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
        let shape = self.operand_shape(receiver);
        let receiver = self.operand(receiver, span)?;
        let kind = match target {
            MirFieldTarget::RecordSlot { field, .. } => {
                let field = self
                    .program
                    .targets()
                    .field(*field)
                    .ok_or(MirBackendError::MissingTarget("field"))?;
                UnlinkedInstructionKind::GetRecordSlot {
                    dst,
                    record: receiver,
                    field: field.name.clone(),
                    slot: self.stable_field_slot(field.id)?,
                }
            }
            MirFieldTarget::VariantSlot { field, .. } => {
                let field = self
                    .program
                    .targets()
                    .field(*field)
                    .ok_or(MirBackendError::MissingTarget("field"))?;
                UnlinkedInstructionKind::GetEnumSlot {
                    dst,
                    value: receiver,
                    field: field.name.clone(),
                    slot: self.stable_field_slot(field.id)?,
                }
            }
            MirFieldTarget::DynamicRecord { name } => {
                if let Some((slot, _)) = self.shape_field(shape.as_ref(), name, false) {
                    UnlinkedInstructionKind::GetRecordSlot {
                        dst,
                        record: receiver,
                        field: name.clone(),
                        slot,
                    }
                } else if let Some((slot, _)) = self.shape_field(shape.as_ref(), name, true) {
                    UnlinkedInstructionKind::GetEnumSlot {
                        dst,
                        value: receiver,
                        field: name.clone(),
                        slot,
                    }
                } else {
                    UnlinkedInstructionKind::GetRecordField {
                        dst,
                        record: receiver,
                        field: name.clone(),
                    }
                }
            }
            MirFieldTarget::DynamicVariant { name } => {
                if let Some((slot, _)) = self.shape_field(shape.as_ref(), name, true) {
                    UnlinkedInstructionKind::GetEnumSlot {
                        dst,
                        value: receiver,
                        field: name.clone(),
                        slot,
                    }
                } else {
                    UnlinkedInstructionKind::GetEnumField {
                        dst,
                        value: receiver,
                        field: name.clone(),
                    }
                }
            }
        };
        self.emit(kind, span);
        Ok(())
    }

    fn write_field(
        &mut self,
        receiver: &MirOperand,
        target: &MirFieldTarget,
        value: &MirOperand,
        span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
        let shape = self.operand_shape(receiver);
        let record = self.operand(receiver, span)?;
        let src = self.operand(value, span)?;
        let kind = match target {
            MirFieldTarget::RecordSlot { field, .. } => {
                let field = self
                    .program
                    .targets()
                    .field(*field)
                    .ok_or(MirBackendError::MissingTarget("field"))?;
                UnlinkedInstructionKind::SetRecordSlot {
                    record,
                    field: field.name.clone(),
                    slot: self.stable_field_slot(field.id)?,
                    src,
                }
            }
            MirFieldTarget::DynamicRecord { name } => {
                if let Some((slot, _)) = self.shape_field(shape.as_ref(), name, false) {
                    UnlinkedInstructionKind::SetRecordSlot {
                        record,
                        field: name.clone(),
                        slot,
                        src,
                    }
                } else {
                    UnlinkedInstructionKind::SetRecordField {
                        record,
                        field: name.clone(),
                        src,
                    }
                }
            }
            MirFieldTarget::VariantSlot { .. } | MirFieldTarget::DynamicVariant { .. } => {
                return Err(MirBackendError::MissingTarget(
                    "verified writable record field",
                ));
            }
        };
        self.emit(kind, span);
        Ok(())
    }

    fn index(
        &mut self,
        dst: Option<Register>,
        operation: &MirIndexOperation,
        span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
        let kind = match operation {
            MirIndexOperation::Read { receiver, index } => {
                let base = self.operand(receiver, span)?;
                match index {
                    MirIndexKey::Value(index) => UnlinkedInstructionKind::GetIndex {
                        dst: dst.ok_or(MirBackendError::MissingDestination)?,
                        base,
                        index: self.operand(index, span)?,
                    },
                    MirIndexKey::ConstantString(key) => {
                        let key = self.code.push_constant(Constant::String(key.clone()));
                        UnlinkedInstructionKind::GetStringKeyIndex {
                            dst: dst.ok_or(MirBackendError::MissingDestination)?,
                            base,
                            key,
                        }
                    }
                }
            }
            MirIndexOperation::Write {
                receiver,
                index,
                value,
            } => {
                let base = self.operand(receiver, span)?;
                let src = self.operand(value, span)?;
                match index {
                    MirIndexKey::Value(index) => UnlinkedInstructionKind::SetIndex {
                        base,
                        index: self.operand(index, span)?,
                        src,
                    },
                    MirIndexKey::ConstantString(key) => {
                        let key = self.code.push_constant(Constant::String(key.clone()));
                        UnlinkedInstructionKind::SetStringKeyIndex { base, key, src }
                    }
                }
            }
        };
        self.emit(kind, span);
        Ok(())
    }
}
