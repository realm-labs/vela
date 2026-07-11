use std::collections::{BTreeMap, BTreeSet};

use vela_host::resolved::HostMutationOp;
use vela_host::target::HostTargetPlan;
use vela_mir::{
    CompileTryFamily, CompileTryTarget, DebugLocalKind, MirAggregate, MirBackendHandoff,
    MirBinaryOp, MirBlockId, MirCall, MirComparisonOp, MirContextualBinaryOp, MirDynamicBinaryOp,
    MirDynamicUnaryOp, MirFieldTarget, MirFormatPart, MirFunction, MirFunctionId,
    MirGlobalOperation, MirGuardAssumption, MirGuardLocation, MirHostMutation, MirHostOperation,
    MirHostPath, MirHostPathSegment, MirIdentityOp, MirImmediate, MirIndexKey, MirIndexOperation,
    MirIteratorOperation, MirLiteralSide, MirNumericBinaryOp, MirOperand, MirPatternPredicate,
    MirLiveValue, MirPlace, MirProgram, MirReflectionOperation, MirRvalue,
    MirScriptParameterGuardMode, MirStatementId, MirStatementKind, MirSwitchValue,
    MirTerminatorKind, MirTypeContract, MirUnaryOp,
};

use crate::{
    BinaryLiteralOp, BinaryLiteralSide, CacheSiteId, CallArgument, Constant, DynamicCallArgument,
    FormatStringPart, FrameSlotInfo, FrameSlotKind, FunctionIndex, GuardKind, GuardLocation,
    InstructionOffset, Register, ScriptCallMode, StandardTypeGuard, TryPropagateFamily,
    UnlinkedCodeObject, UnlinkedGuardContext, UnlinkedInstruction, UnlinkedInstructionKind,
    UnlinkedParameterTypeGuard, UnlinkedTypeGuard, UnlinkedTypeGuardPlan,
};

use super::cache_sites::{attach_cache_site, cache_site_kind};
use super::constant_encoding::encode_evaluated_constant;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MirBackendError {
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
) -> Result<UnlinkedCodeObject, MirBackendError> {
    let program = handoff.program();
    let (root_id, root) = program
        .functions()
        .next()
        .ok_or(MirBackendError::MissingRoot)?;
    compile_function(program, root_id, root)
}

fn compile_function(
    program: &MirProgram,
    function_id: MirFunctionId,
    function: &MirFunction,
) -> Result<UnlinkedCodeObject, MirBackendError> {
    let mut backend = FunctionBackend::new(program, function_id, function)?;
    backend.compile()?;
    Ok(backend.finish())
}

struct FunctionBackend<'a> {
    program: &'a MirProgram,
    function_id: MirFunctionId,
    function: &'a MirFunction,
    code: UnlinkedCodeObject,
    locals: BTreeMap<vela_mir::MirLocalId, Register>,
    temps: BTreeMap<vela_mir::MirTempId, Register>,
    blocks: BTreeMap<MirBlockId, InstructionOffset>,
    patches: Vec<(usize, MirBlockId)>,
    next_register: u16,
    nested: BTreeMap<MirFunctionId, FunctionIndex>,
    shapes: BTreeMap<Register, PhysicalShape>,
    immediates: BTreeMap<Register, MirImmediate>,
    missing_tests: BTreeMap<Register, Register>,
    known_false: BTreeSet<Register>,
    skipped_blocks: BTreeSet<MirBlockId>,
    loop_blocks: BTreeSet<MirBlockId>,
    try_join_blocks: BTreeSet<MirBlockId>,
    temp_aliases: BTreeMap<vela_mir::MirTempId, Register>,
    current_block: Option<MirBlockId>,
    current_statement: Option<MirStatementId>,
    last_statement: Option<(MirStatementId, Option<MirPlace>, usize, usize)>,
    unreachable_padding: Option<vela_common::Span>,
    unspanned_spans: Vec<vela_common::Span>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PhysicalShape {
    Record(BTreeMap<String, (usize, Option<Box<PhysicalShape>>)>),
    Variant(BTreeMap<String, (usize, Option<Box<PhysicalShape>>)>),
    Array(Option<Box<PhysicalShape>>),
}

impl<'a> FunctionBackend<'a> {
    fn new(
        program: &'a MirProgram,
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
            function_id,
            function,
            code,
            locals,
            temps,
            blocks: BTreeMap::new(),
            patches: Vec::new(),
            next_register,
            nested: BTreeMap::new(),
            shapes: BTreeMap::new(),
            immediates: BTreeMap::new(),
            missing_tests: BTreeMap::new(),
            known_false: BTreeSet::new(),
            skipped_blocks: BTreeSet::new(),
            loop_blocks,
            try_join_blocks,
            temp_aliases: BTreeMap::new(),
            current_block: None,
            current_statement: None,
            last_statement: None,
            unreachable_padding: None,
            unspanned_spans,
        })
    }

    fn compile(&mut self) -> Result<(), MirBackendError> {
        self.compile_nested_functions()?;
        let blocks = self.function.blocks().collect::<Vec<_>>();
        for (index, (block_id, block)) in blocks.iter().copied().enumerate() {
            if self.skipped_blocks.contains(&block_id) {
                continue;
            }
            self.current_block = Some(block_id);
            let next = blocks.get(index + 1).map(|(id, _)| *id);
            self.blocks
                .insert(block_id, InstructionOffset(self.code.instructions.len()));
            for statement_id in block.statements() {
                let statement = self
                    .function
                    .statement(*statement_id)
                    .ok_or(MirBackendError::MissingStatement)?;
                let start = self.code.instructions.len();
                self.current_statement = Some(*statement_id);
                self.statement(statement)?;
                self.last_statement = Some((
                    *statement_id,
                    statement.destination,
                    start,
                    self.code.instructions.len(),
                ));
            }
            self.current_statement = None;
            let terminator = block
                .terminator()
                .ok_or(MirBackendError::MissingBlock(block_id))?;
            self.terminator(&terminator.kind, terminator.origin.span, next)?;
        }
        if let Some(span) = self.unreachable_padding {
            let src = self.alloc_register()?;
            self.load_immediate(src, MirImmediate::Unit, span);
            self.emit(UnlinkedInstructionKind::Return { src }, span);
        }
        self.patch_targets()?;
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
            let code = compile_function(self.program, id, function)?;
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
        if let Some(dst) = dst {
            self.materialize_aliases_before_write(dst, statement.origin.span);
            self.immediates.remove(&dst);
            self.shapes.remove(&dst);
            self.missing_tests.remove(&dst);
            self.known_false.remove(&dst);
        }
        let span = statement.origin.span;
        match &statement.kind {
            MirStatementKind::Assign(value) => {
                self.assign(dst.ok_or(MirBackendError::MissingDestination)?, value, span)?
            }
            MirStatementKind::Unary { operation, operand } => {
                let src = self.operand(operand, span)?;
                let kind = match operation {
                    MirUnaryOp::NotBool if self.invert_last_comparison(src, dst.ok_or(MirBackendError::MissingDestination)?) => {
                        return Ok(());
                    }
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
                let kind = self.select_binary(*operation, dst, lhs, rhs);
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
            MirStatementKind::ReadField { receiver, target } => {
                self.read_field(dst.ok_or(MirBackendError::MissingDestination)?, receiver, target, span)?
            }
            MirStatementKind::WriteField {
                receiver,
                target,
                value,
            } => self.write_field(receiver, target, value, span)?,
            MirStatementKind::Index(operation) => self.index(dst, operation, span)?,
            MirStatementKind::Global(MirGlobalOperation::Read { global }) => {
                let target = self
                    .program
                    .targets()
                    .global(*global)
                    .ok_or(MirBackendError::MissingTarget("global"))?;
                self.emit(
                    UnlinkedInstructionKind::LoadGlobal {
                        dst: dst.ok_or(MirBackendError::MissingDestination)?,
                        global: target.name.clone(),
                        slot: self.global_slot(*global),
                        cache_site: None,
                    },
                    span,
                );
            }
            MirStatementKind::Allocate(aggregate) => {
                self.allocate(dst.ok_or(MirBackendError::MissingDestination)?, aggregate, span)?
            }
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
            MirStatementKind::Call(call) => self.call(dst.ok_or(MirBackendError::MissingDestination)?, call, span)?,
            MirStatementKind::Host(operation) => self.host(dst, operation, span)?,
            MirStatementKind::Reflect(operation) => self.reflect(dst.ok_or(MirBackendError::MissingDestination)?, operation, span)?,
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
            MirStatementKind::Iterator(MirIteratorOperation::Create { iterable }) => {
                let iterable = self.operand(iterable, span)?;
                self.emit(
                    UnlinkedInstructionKind::IterInit {
                        dst: dst.ok_or(MirBackendError::MissingDestination)?,
                        iterable,
                    },
                    span,
                );
            }
        }
        Ok(())
    }

    fn assign(
        &mut self,
        dst: Register,
        value: &MirRvalue,
        span: vela_common::Span,
    ) -> Result<(), MirBackendError> {
        match value {
            MirRvalue::Use(value) => {
                if self
                    .current_block
                    .is_some_and(|block| self.try_join_blocks.contains(&block))
                    && let Some(temp) = self.temp_for_register(dst)
                {
                    let src = self.operand(value, span)?;
                    self.temp_aliases.insert(temp, src);
                    self.copy_shape(dst, src);
                    self.copy_immediate(dst, src);
                    return Ok(());
                }
                if self.locals.values().any(|register| *register == dst)
                    && let MirOperand::Temp(temp) = value
                    && self.try_retarget_dead_temp(*temp, dst)
                {
                    return Ok(());
                }
                if self.locals.values().any(|register| *register == dst)
                    && let MirOperand::Local(local) = value
                    && self.try_retarget_try_result(*local, dst)
                {
                    return Ok(());
                }
                let src = self.operand(value, span)?;
                if src != dst {
                    self.emit(UnlinkedInstructionKind::Move { dst, src }, span);
                }
                self.copy_shape(dst, src);
                self.copy_immediate(dst, src);
            }
            MirRvalue::Constant { value, .. } => self.load_immediate(dst, *value, span),
            MirRvalue::Truthy { value } => {
                let src = self.operand(value, span)?;
                self.emit(UnlinkedInstructionKind::Truthy { dst, src }, span);
            }
            MirRvalue::IsMissing { value } => {
                let src = self.operand(value, span)?;
                self.missing_tests.insert(dst, src);
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
                if !self.remove_dead_temp_move(value) {
                    let _ = self.operand(value, span)?;
                }
                self.load_immediate(dst, MirImmediate::Bool(false), span);
                self.known_false.insert(dst);
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
                        variant: variant.name.clone(),
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
                        variant: variant_name.clone(),
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
                let shape = elements
                    .first()
                    .and_then(|register| self.shapes.get(register).cloned())
                    .filter(|shape| {
                        elements
                            .iter()
                            .all(|register| self.shapes.get(register) == Some(shape))
                    });
                self.shapes
                    .insert(dst, PhysicalShape::Array(shape.map(Box::new)));
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
                self.install_record_shape(dst, &fields, false);
                UnlinkedInstructionKind::MakeRecord {
                    dst,
                    type_name: ty.runtime_name.clone(),
                    fields,
                }
            }
            MirAggregate::DynamicRecord { type_name, fields } => {
                let fields = fields
                    .iter()
                    .map(|(name, value)| Ok((name.clone(), self.operand(value, span)?)))
                    .collect::<Result<Vec<_>, MirBackendError>>()?;
                self.install_record_shape(dst, &fields, false);
                UnlinkedInstructionKind::MakeRecord {
                    dst,
                    type_name: type_name.clone(),
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
                self.install_record_shape(dst, &fields, true);
                UnlinkedInstructionKind::MakeEnum {
                    dst,
                    enum_name: ty.runtime_name.clone(),
                    variant: variant.name.clone(),
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
                self.install_record_shape(dst, &fields, true);
                UnlinkedInstructionKind::MakeEnum {
                    dst,
                    enum_name: owner_name.clone(),
                    variant: variant_name.clone(),
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
                if let Some((slot, shape)) = self.shape_field(receiver, name, false) {
                    if let Some(shape) = shape {
                        self.shapes.insert(dst, shape);
                    }
                    UnlinkedInstructionKind::GetRecordSlot {
                        dst,
                        record: receiver,
                        field: name.clone(),
                        slot,
                    }
                } else if let Some((slot, shape)) = self.shape_field(receiver, name, true) {
                    if let Some(shape) = shape {
                        self.shapes.insert(dst, shape);
                    }
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
                if let Some((slot, shape)) = self.shape_field(receiver, name, true) {
                    if let Some(shape) = shape {
                        self.shapes.insert(dst, shape);
                    }
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
                if let Some((slot, _)) = self.shape_field(record, name, false) {
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
                if let Some(PhysicalShape::Array(Some(element))) = self.shapes.get(&base).cloned() {
                    self.shapes.insert(dst.ok_or(MirBackendError::MissingDestination)?, *element);
                }
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
