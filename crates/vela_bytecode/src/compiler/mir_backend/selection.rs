use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use vela_mir::{
    MirBackendHandoff, MirBasicBlock, MirBinaryOp, MirBlockId, MirBudgetPoint, MirBudgetSite,
    MirComparisonOp, MirDebugLocalId, MirFunction, MirFunctionAnalyses, MirFunctionId,
    MirImmediate, MirLiveValue, MirNumericBinaryOp, MirOperand, MirPlace, MirRangeStepMode,
    MirRvalue, MirSafepointId, MirSourceOrigin, MirStatementId, MirStatementKind,
    MirTerminatorKind, MirUnaryOp, MirValueType,
};

mod verify;

pub(super) fn verify(
    handoff: MirBackendHandoff<'_>,
    plan: &SelectedProgramPlan,
) -> Result<(), SelectionError> {
    verify::verify(handoff, plan)
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SelectedProgramPlan {
    functions: BTreeMap<MirFunctionId, SelectedFunctionPlan>,
}

impl SelectedProgramPlan {
    pub(super) fn function(&self, function: MirFunctionId) -> Option<&SelectedFunctionPlan> {
        self.functions.get(&function)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SelectedFunctionPlan {
    function: MirFunctionId,
    units: Box<[SelectedUnit]>,
    block_entries: BTreeMap<MirBlockId, SelectedUnitId>,
    coverage: Box<[SelectedCoverage]>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SelectedUnitId(u32);

#[derive(Clone, Debug, PartialEq)]
enum SelectedUnit {
    Ordinary(MirUnitRange),
    Superinstruction(SuperinstructionPlan),
    ScalarBlock(ScalarBlockSelection),
}

impl SelectedUnit {
    const fn block(&self) -> MirBlockId {
        match self {
            Self::Ordinary(range) => range.block,
            Self::Superinstruction(plan) => plan.block,
            Self::ScalarBlock(plan) => plan.block,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SuperinstructionPlan {
    block: MirBlockId,
    statement: MirStatementId,
    op: crate::I64CompareOp,
    value: MirOperand,
    immediate: i64,
    then_block: MirBlockId,
    else_block: MirBlockId,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ScalarBlockSelection {
    block: MirBlockId,
    statements: Box<[MirStatementId]>,
    range_loop: Option<ScalarRangeLoopSelection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ScalarRangeLoopSelection {
    header: MirBlockId,
}

impl SelectedFunctionPlan {
    pub(super) fn superinstruction(&self, block: MirBlockId) -> Option<&SuperinstructionPlan> {
        self.block_entries
            .get(&block)
            .and_then(|unit| self.units.get(unit.0 as usize))
            .and_then(|unit| match unit {
                SelectedUnit::Superinstruction(plan) => Some(plan),
                SelectedUnit::Ordinary(_) | SelectedUnit::ScalarBlock(_) => None,
            })
    }

    pub(super) fn scalar_block(&self, block: MirBlockId) -> Option<&ScalarBlockSelection> {
        self.block_entries
            .get(&block)
            .and_then(|unit| self.units.get(unit.0 as usize))
            .and_then(|unit| match unit {
                SelectedUnit::ScalarBlock(plan) => Some(plan),
                SelectedUnit::Ordinary(_) | SelectedUnit::Superinstruction(_) => None,
            })
    }
}

impl ScalarBlockSelection {
    pub(super) const fn block(&self) -> MirBlockId {
        self.block
    }

    pub(super) const fn statements(&self) -> &[MirStatementId] {
        &self.statements
    }

    pub(super) const fn range_loop(&self) -> Option<ScalarRangeLoopSelection> {
        self.range_loop
    }
}

impl ScalarRangeLoopSelection {
    pub(super) const fn header(self) -> MirBlockId {
        self.header
    }
}

impl SuperinstructionPlan {
    pub(super) const fn statement(&self) -> MirStatementId {
        self.statement
    }

    pub(super) const fn op(&self) -> crate::I64CompareOp {
        self.op
    }

    pub(super) const fn value(&self) -> &MirOperand {
        &self.value
    }

    pub(super) const fn immediate(&self) -> i64 {
        self.immediate
    }

    pub(super) const fn then_block(&self) -> MirBlockId {
        self.then_block
    }

    pub(super) const fn else_block(&self) -> MirBlockId {
        self.else_block
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MirUnitRange {
    block: MirBlockId,
    reason: OrdinaryReason,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum OrdinaryReason {
    NoApprovedRecipe,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SelectionCandidate {
    I64CompareImmediateBranch,
    ScalarBlock,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedCoverage {
    function: MirFunctionId,
    block: MirBlockId,
    statements: Box<[MirStatementId]>,
    terminator: MirBlockId,
    budget: Box<[SelectedBudgetCoverage]>,
    safepoints: Box<[SelectedSafepointCoverage]>,
    exits: Box<[SelectedExitCoverage]>,
    source_points: Box<[SelectedSourcePoint]>,
    live_in: BTreeSet<MirLiveValue>,
    live_out: BTreeSet<MirLiveValue>,
    debug_at_entry: BTreeSet<MirDebugLocalId>,
    statement_liveness: Box<[SelectedStatementLiveness]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SelectedBudgetCoverage {
    site: MirBudgetSite,
    point: MirBudgetPoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedSafepointCoverage {
    safepoint: MirSafepointId,
    live_roots: BTreeSet<MirLiveValue>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SelectedExitCoverage {
    target: MirBlockId,
    budget: Option<MirBudgetPoint>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectedSourcePointKind {
    Statement(MirStatementId),
    Terminator(MirBlockId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SelectedSourcePoint {
    kind: SelectedSourcePointKind,
    origin: MirSourceOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedStatementLiveness {
    statement: MirStatementId,
    live_before: BTreeSet<MirLiveValue>,
    live_after: BTreeSet<MirLiveValue>,
    debug_before: BTreeSet<MirDebugLocalId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SelectionError {
    MissingAnalysis(MirFunctionId),
    MissingFunctionPlan(MirFunctionId),
    UnexpectedFunctionPlan(MirFunctionId),
    FunctionMismatch {
        expected: MirFunctionId,
        actual: MirFunctionId,
    },
    UnitCountOverflow(MirFunctionId),
    UnitCoverageCountMismatch(MirFunctionId),
    MissingBlockEntry {
        function: MirFunctionId,
        block: MirBlockId,
    },
    InvalidBlockEntry {
        function: MirFunctionId,
        block: MirBlockId,
    },
    MissingBlock {
        function: MirFunctionId,
        block: MirBlockId,
    },
    DuplicateBlock {
        function: MirFunctionId,
        block: MirBlockId,
    },
    UnitBlockMismatch {
        function: MirFunctionId,
        unit: MirBlockId,
        coverage: MirBlockId,
    },
    StatementCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    TerminatorCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    BudgetCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    SafepointCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    ExitCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    SourceCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    LivenessCoverageMismatch {
        function: MirFunctionId,
        block: MirBlockId,
    },
    MissingAnalysisFact {
        function: MirFunctionId,
        block: MirBlockId,
        fact: &'static str,
    },
    InvalidSuperinstruction {
        function: MirFunctionId,
        block: MirBlockId,
    },
    InvalidScalarBlock {
        function: MirFunctionId,
        block: MirBlockId,
    },
}

impl SelectionError {
    pub(crate) const fn function(&self) -> Option<MirFunctionId> {
        match *self {
            Self::MissingAnalysis(function)
            | Self::MissingFunctionPlan(function)
            | Self::UnexpectedFunctionPlan(function)
            | Self::UnitCountOverflow(function)
            | Self::UnitCoverageCountMismatch(function) => Some(function),
            Self::FunctionMismatch { expected, .. }
            | Self::MissingBlockEntry {
                function: expected, ..
            }
            | Self::InvalidBlockEntry {
                function: expected, ..
            }
            | Self::MissingBlock {
                function: expected, ..
            }
            | Self::DuplicateBlock {
                function: expected, ..
            }
            | Self::UnitBlockMismatch {
                function: expected, ..
            }
            | Self::StatementCoverageMismatch {
                function: expected, ..
            }
            | Self::TerminatorCoverageMismatch {
                function: expected, ..
            }
            | Self::BudgetCoverageMismatch {
                function: expected, ..
            }
            | Self::SafepointCoverageMismatch {
                function: expected, ..
            }
            | Self::ExitCoverageMismatch {
                function: expected, ..
            }
            | Self::SourceCoverageMismatch {
                function: expected, ..
            }
            | Self::LivenessCoverageMismatch {
                function: expected, ..
            }
            | Self::MissingAnalysisFact {
                function: expected, ..
            }
            | Self::InvalidSuperinstruction {
                function: expected, ..
            }
            | Self::InvalidScalarBlock {
                function: expected, ..
            } => Some(expected),
        }
    }
}

impl fmt::Display for SelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid verified-MIR selection coverage: {self:?}"
        )
    }
}

pub(super) fn select(
    handoff: MirBackendHandoff<'_>,
) -> Result<SelectedProgramPlan, SelectionError> {
    let mut functions = BTreeMap::new();
    for (function_id, function) in handoff.program().functions() {
        let analyses = handoff
            .analyses(function_id)
            .ok_or(SelectionError::MissingAnalysis(function_id))?;
        let mut units = Vec::new();
        let mut coverage = Vec::new();
        let mut block_entries = BTreeMap::new();
        for (block, _) in function.blocks() {
            let unit = SelectedUnitId(
                u32::try_from(units.len())
                    .map_err(|_| SelectionError::UnitCountOverflow(function_id))?,
            );
            units.push(
                measured_candidate(function, analyses, block)
                    .map(SelectedUnit::Superinstruction)
                    .or_else(|| {
                        scalar_block_candidate(function, analyses, block)
                            .map(SelectedUnit::ScalarBlock)
                    })
                    .unwrap_or({
                        SelectedUnit::Ordinary(MirUnitRange {
                            block,
                            reason: OrdinaryReason::NoApprovedRecipe,
                        })
                    }),
            );
            coverage.push(select_coverage(function_id, function, analyses, block)?);
            block_entries.insert(block, unit);
        }
        functions.insert(
            function_id,
            SelectedFunctionPlan {
                function: function_id,
                units: units.into_boxed_slice(),
                block_entries,
                coverage: coverage.into_boxed_slice(),
            },
        );
    }
    Ok(SelectedProgramPlan { functions })
}

fn scalar_block_candidate(
    function: &MirFunction,
    analyses: &MirFunctionAnalyses,
    block_id: MirBlockId,
) -> Option<ScalarBlockSelection> {
    // The first scalar-block tier is intentionally profile-backed: cold
    // straight-line setup blocks add plan metadata and perturb unrelated
    // dispatch without amortizing the extra physical dispatch. Batch E owns
    // broader loop-region formation; Batch D selects only blocks in a CFG
    // cycle after the shorter superinstruction recipes have had priority.
    if !block_is_cyclic(function, block_id) {
        return None;
    }
    let block = function.block(block_id)?;
    if block.statements().len() < 3
        || block
            .statements()
            .iter()
            .any(|statement| !scalar_statement(function, analyses, *statement))
    {
        return None;
    }
    let terminator = block.terminator()?;
    if terminator.safepoint.is_some() {
        return None;
    }
    match &terminator.kind {
        MirTerminatorKind::Jump(_) => {}
        MirTerminatorKind::Branch { condition, .. } => {
            let last = *block.statements().last()?;
            let statement = function.statement(last)?;
            if statement.destination != operand_place(condition)
                || !statement_produces_bool(&statement.kind)
            {
                return None;
            }
        }
        _ => return None,
    }
    Some(ScalarBlockSelection {
        block: block_id,
        statements: block.statements().to_vec().into_boxed_slice(),
        range_loop: scalar_range_loop(function, block_id),
    })
}

fn scalar_range_loop(
    function: &MirFunction,
    latch: MirBlockId,
) -> Option<ScalarRangeLoopSelection> {
    let MirTerminatorKind::Jump(header) = function.block(latch)?.terminator()?.kind else {
        return None;
    };
    let header_terminator = function.block(header)?.terminator()?;
    let MirTerminatorKind::RangeNext {
        mode: MirRangeStepMode::I64Proven,
        next,
        done,
        ..
    } = header_terminator.kind
    else {
        return None;
    };
    if header_terminator.safepoint.is_some() || next != latch || done == header || done == latch {
        return None;
    }
    let predecessors = mir_predecessors(function);
    if predecessors.get(&latch).map(Vec::as_slice) != Some([header].as_slice()) {
        return None;
    }
    let dominators = mir_dominators(function, &predecessors);
    let cyclic_predecessors = predecessors
        .get(&header)?
        .iter()
        .filter(|predecessor| {
            dominators
                .get(predecessor)
                .is_some_and(|blocks| blocks.contains(&header))
        })
        .copied()
        .collect::<Vec<_>>();
    if cyclic_predecessors.as_slice() != [latch] {
        return None;
    }
    Some(ScalarRangeLoopSelection { header })
}

fn mir_predecessors(function: &MirFunction) -> BTreeMap<MirBlockId, Vec<MirBlockId>> {
    let mut predecessors = BTreeMap::<MirBlockId, Vec<MirBlockId>>::new();
    for (block, data) in function.blocks() {
        if let Some(terminator) = data.terminator() {
            for successor in mir_successors(&terminator.kind) {
                predecessors.entry(successor).or_default().push(block);
            }
        }
    }
    predecessors
}

fn mir_dominators(
    function: &MirFunction,
    predecessors: &BTreeMap<MirBlockId, Vec<MirBlockId>>,
) -> BTreeMap<MirBlockId, BTreeSet<MirBlockId>> {
    let blocks = function
        .blocks()
        .map(|(block, _)| block)
        .collect::<BTreeSet<_>>();
    let entry = function.entry_block();
    let mut dominators = blocks
        .iter()
        .map(|block| {
            (
                *block,
                if *block == entry {
                    BTreeSet::from([entry])
                } else {
                    blocks.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in blocks.iter().copied().filter(|block| *block != entry) {
            let mut next = predecessors
                .get(&block)
                .and_then(|items| items.first())
                .and_then(|predecessor| dominators.get(predecessor))
                .cloned()
                .unwrap_or_default();
            if let Some(items) = predecessors.get(&block) {
                for predecessor in items.iter().skip(1) {
                    if let Some(other) = dominators.get(predecessor) {
                        next.retain(|candidate| other.contains(candidate));
                    }
                }
            }
            next.insert(block);
            if dominators.get(&block) != Some(&next) {
                dominators.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return dominators;
        }
    }
}

fn block_is_cyclic(function: &MirFunction, origin: MirBlockId) -> bool {
    let Some(terminator) = function.block(origin).and_then(MirBasicBlock::terminator) else {
        return false;
    };
    let mut pending = mir_successors(&terminator.kind);
    let mut visited = BTreeSet::new();
    while let Some(block) = pending.pop() {
        if block == origin {
            return true;
        }
        if !visited.insert(block) {
            continue;
        }
        if let Some(terminator) = function.block(block).and_then(MirBasicBlock::terminator) {
            pending.extend(mir_successors(&terminator.kind));
        }
    }
    false
}

fn scalar_statement(
    function: &MirFunction,
    analyses: &MirFunctionAnalyses,
    statement_id: MirStatementId,
) -> bool {
    let Some(statement) = function.statement(statement_id) else {
        return false;
    };
    if statement.safepoint.is_some() || statement.destination.is_none() {
        return false;
    }
    match &statement.kind {
        MirStatementKind::Assign(MirRvalue::Use(operand)) => analyses
            .facts
            .operand_before(statement_id, operand)
            .is_some_and(|fact| {
                matches!(
                    fact.value_type,
                    MirValueType::Primitive(
                        vela_common::PrimitiveTag::Bool | vela_common::PrimitiveTag::I64
                    )
                )
            }),
        MirStatementKind::Assign(MirRvalue::Constant { value, .. }) => {
            matches!(
                value,
                MirImmediate::Bool(_) | MirImmediate::Scalar(vela_common::ScalarValue::I64(_))
            )
        }
        MirStatementKind::Unary {
            operation: MirUnaryOp::NotBool,
            operand,
        } => register_operand(operand),
        MirStatementKind::Binary {
            operation:
                MirBinaryOp::Numeric {
                    operation:
                        MirNumericBinaryOp::Add
                        | MirNumericBinaryOp::Subtract
                        | MirNumericBinaryOp::Multiply
                        | MirNumericBinaryOp::Remainder,
                    kind: vela_common::NumericTag::I64,
                }
                | MirBinaryOp::Compare {
                    kind: vela_common::PrimitiveTag::I64,
                    ..
                },
            left,
            right,
        } => register_operand(left) && register_operand(right),
        _ => false,
    }
}

const fn register_operand(operand: &MirOperand) -> bool {
    matches!(operand, MirOperand::Local(_) | MirOperand::Temp(_))
}

const fn operand_place(operand: &MirOperand) -> Option<MirPlace> {
    match operand {
        MirOperand::Local(local) => Some(MirPlace::Local(*local)),
        MirOperand::Temp(temp) => Some(MirPlace::Temp(*temp)),
        MirOperand::Immediate(_) => None,
    }
}

const fn statement_produces_bool(statement: &MirStatementKind) -> bool {
    matches!(
        statement,
        MirStatementKind::Unary {
            operation: MirUnaryOp::NotBool,
            ..
        } | MirStatementKind::Binary {
            operation: MirBinaryOp::Compare { .. },
            ..
        } | MirStatementKind::Assign(MirRvalue::Use(MirOperand::Immediate(MirImmediate::Bool(_))))
            | MirStatementKind::Assign(MirRvalue::Constant {
                value: MirImmediate::Bool(_),
                ..
            })
    )
}

fn measured_candidate(
    function: &MirFunction,
    analyses: &MirFunctionAnalyses,
    block_id: MirBlockId,
) -> Option<SuperinstructionPlan> {
    let block = function.block(block_id)?;
    let last_statement_id = block.statements().last().copied()?;
    let last_statement = function.statement(last_statement_id)?;
    let MirStatementKind::Binary {
        operation:
            MirBinaryOp::Compare {
                operation,
                kind: vela_common::PrimitiveTag::I64,
            },
        left,
        right,
    } = &last_statement.kind
    else {
        return None;
    };
    let MirPlace::Temp(destination) = last_statement.destination? else {
        return None;
    };
    let MirTerminatorKind::Branch {
        condition: vela_mir::MirOperand::Temp(condition),
        ..
    } = &block.terminator()?.kind
    else {
        return None;
    };
    if destination != *condition {
        return None;
    }
    if analyses
        .value_liveness
        .block_live_out
        .get(&block_id)
        .is_none_or(|live| live.contains(&MirLiveValue::Temp(destination)))
        || last_statement.safepoint.is_some()
        || block.terminator()?.safepoint.is_some()
    {
        return None;
    }
    let (value, immediate, operation) =
        compare_immediate_operands(analyses, last_statement_id, *operation, left, right)?;
    let MirTerminatorKind::Branch {
        then_block,
        else_block,
        ..
    } = block.terminator()?.kind
    else {
        unreachable!("branch shape was checked above")
    };
    Some(SuperinstructionPlan {
        block: block_id,
        statement: last_statement_id,
        op: bytecode_compare(operation),
        value,
        immediate,
        then_block,
        else_block,
    })
}

fn compare_immediate_operands(
    analyses: &MirFunctionAnalyses,
    statement: MirStatementId,
    operation: MirComparisonOp,
    left: &MirOperand,
    right: &MirOperand,
) -> Option<(MirOperand, i64, MirComparisonOp)> {
    let left_immediate = i64_immediate(analyses, statement, left);
    let right_immediate = i64_immediate(analyses, statement, right);
    match (left_immediate, right_immediate) {
        (None, Some(immediate)) if !matches!(left, MirOperand::Immediate(_)) => {
            Some((left.clone(), immediate, operation))
        }
        (Some(immediate), None) if !matches!(right, MirOperand::Immediate(_)) => {
            Some((right.clone(), immediate, reverse_compare(operation)))
        }
        _ => None,
    }
}

fn i64_immediate(
    analyses: &MirFunctionAnalyses,
    statement: MirStatementId,
    operand: &MirOperand,
) -> Option<i64> {
    match analyses
        .facts
        .operand_before(statement, operand)?
        .immediate?
    {
        MirImmediate::Scalar(vela_common::ScalarValue::I64(value)) => Some(value),
        _ => None,
    }
}

const fn reverse_compare(operation: MirComparisonOp) -> MirComparisonOp {
    match operation {
        MirComparisonOp::Equal => MirComparisonOp::Equal,
        MirComparisonOp::NotEqual => MirComparisonOp::NotEqual,
        MirComparisonOp::Less => MirComparisonOp::Greater,
        MirComparisonOp::LessEqual => MirComparisonOp::GreaterEqual,
        MirComparisonOp::Greater => MirComparisonOp::Less,
        MirComparisonOp::GreaterEqual => MirComparisonOp::LessEqual,
    }
}

const fn bytecode_compare(operation: MirComparisonOp) -> crate::I64CompareOp {
    match operation {
        MirComparisonOp::Equal => crate::I64CompareOp::Equal,
        MirComparisonOp::NotEqual => crate::I64CompareOp::NotEqual,
        MirComparisonOp::Less => crate::I64CompareOp::Less,
        MirComparisonOp::LessEqual => crate::I64CompareOp::LessEqual,
        MirComparisonOp::Greater => crate::I64CompareOp::Greater,
        MirComparisonOp::GreaterEqual => crate::I64CompareOp::GreaterEqual,
    }
}

fn select_coverage(
    function_id: MirFunctionId,
    function: &MirFunction,
    analyses: &MirFunctionAnalyses,
    block_id: MirBlockId,
) -> Result<SelectedCoverage, SelectionError> {
    let block = function
        .block(block_id)
        .ok_or(SelectionError::MissingBlock {
            function: function_id,
            block: block_id,
        })?;
    let terminator = block
        .terminator()
        .ok_or(SelectionError::TerminatorCoverageMismatch {
            function: function_id,
            block: block_id,
        })?;
    let mut budget = Vec::new();
    let mut safepoints = Vec::new();
    let mut source_points = Vec::new();
    let mut statement_liveness = Vec::new();
    for statement_id in block.statements().iter().copied() {
        let statement =
            function
                .statement(statement_id)
                .ok_or(SelectionError::StatementCoverageMismatch {
                    function: function_id,
                    block: block_id,
                })?;
        if let Some(point) = analyses.budget.statement_before(statement_id) {
            budget.push(SelectedBudgetCoverage {
                site: MirBudgetSite::StatementBefore(statement_id),
                point,
            });
        }
        if let Some(safepoint) = statement.safepoint {
            safepoints.push(safepoint_coverage(
                function_id,
                block_id,
                analyses,
                safepoint,
            )?);
        }
        source_points.push(SelectedSourcePoint {
            kind: SelectedSourcePointKind::Statement(statement_id),
            origin: statement.origin,
        });
        statement_liveness.push(SelectedStatementLiveness {
            statement: statement_id,
            live_before: analysis_set(
                function_id,
                block_id,
                analyses
                    .value_liveness
                    .statement_live_before
                    .get(&statement_id),
                "statement live-before",
            )?,
            live_after: analysis_set(
                function_id,
                block_id,
                analyses
                    .value_liveness
                    .statement_live_after
                    .get(&statement_id),
                "statement live-after",
            )?,
            debug_before: analyses
                .debug_availability
                .statement_before
                .get(&statement_id)
                .cloned()
                .unwrap_or_default(),
        });
    }
    if let Some(point) = analyses.budget.terminator_before(block_id) {
        budget.push(SelectedBudgetCoverage {
            site: MirBudgetSite::TerminatorBefore(block_id),
            point,
        });
    }
    if let Some(safepoint) = terminator.safepoint {
        safepoints.push(safepoint_coverage(
            function_id,
            block_id,
            analyses,
            safepoint,
        )?);
    }
    source_points.push(SelectedSourcePoint {
        kind: SelectedSourcePointKind::Terminator(block_id),
        origin: terminator.origin,
    });
    let exits = mir_successors(&terminator.kind)
        .into_iter()
        .map(|target| {
            let edge_budget = analyses.budget.edge(block_id, target);
            if let Some(point) = edge_budget {
                budget.push(SelectedBudgetCoverage {
                    site: MirBudgetSite::Edge {
                        from: block_id,
                        to: target,
                    },
                    point,
                });
            }
            SelectedExitCoverage {
                target,
                budget: edge_budget,
            }
        })
        .collect::<Vec<_>>();
    Ok(SelectedCoverage {
        function: function_id,
        block: block_id,
        statements: block.statements().to_vec().into_boxed_slice(),
        terminator: block_id,
        budget: budget.into_boxed_slice(),
        safepoints: safepoints.into_boxed_slice(),
        exits: exits.into_boxed_slice(),
        source_points: source_points.into_boxed_slice(),
        live_in: analysis_set(
            function_id,
            block_id,
            analyses.value_liveness.block_live_in.get(&block_id),
            "block live-in",
        )?,
        live_out: analysis_set(
            function_id,
            block_id,
            analyses.value_liveness.block_live_out.get(&block_id),
            "block live-out",
        )?,
        debug_at_entry: analyses
            .debug_availability
            .block_entry
            .get(&block_id)
            .cloned()
            .unwrap_or_default(),
        statement_liveness: statement_liveness.into_boxed_slice(),
    })
}

fn safepoint_coverage(
    function: MirFunctionId,
    block: MirBlockId,
    analyses: &MirFunctionAnalyses,
    safepoint: MirSafepointId,
) -> Result<SelectedSafepointCoverage, SelectionError> {
    let live_roots = analyses
        .root_liveness
        .live_before_safepoint
        .get(&safepoint)
        .cloned()
        .ok_or(SelectionError::MissingAnalysisFact {
            function,
            block,
            fact: "safepoint live roots",
        })?;
    Ok(SelectedSafepointCoverage {
        safepoint,
        live_roots,
    })
}

fn analysis_set<T: Ord + Clone>(
    function: MirFunctionId,
    block: MirBlockId,
    values: Option<&BTreeSet<T>>,
    fact: &'static str,
) -> Result<BTreeSet<T>, SelectionError> {
    values.cloned().ok_or(SelectionError::MissingAnalysisFact {
        function,
        block,
        fact,
    })
}

pub(super) fn mir_successors(kind: &MirTerminatorKind) -> Vec<MirBlockId> {
    match kind {
        MirTerminatorKind::Jump(target) => vec![*target],
        MirTerminatorKind::Branch {
            then_block,
            else_block,
            ..
        } => vec![*then_block, *else_block],
        MirTerminatorKind::Switch {
            cases, otherwise, ..
        } => cases
            .iter()
            .map(|case| case.target)
            .chain(std::iter::once(*otherwise))
            .collect(),
        MirTerminatorKind::GuardBranch { passed, slow, .. } => vec![*passed, *slow],
        MirTerminatorKind::TrySwitch {
            continuations,
            propagate,
            invalid,
            join,
            ..
        } => continuations
            .iter()
            .map(|continuation| continuation.block)
            .chain([*propagate, *invalid, *join])
            .collect(),
        MirTerminatorKind::IteratorNext { next, done, .. }
        | MirTerminatorKind::RangeNext { next, done, .. } => vec![*next, *done],
        MirTerminatorKind::AwaitCall { resume, .. } => vec![*resume],
        MirTerminatorKind::Return(_)
        | MirTerminatorKind::TryTypeMismatch { .. }
        | MirTerminatorKind::Unreachable => Vec::new(),
    }
}

#[cfg(test)]
#[derive(Debug, Eq, PartialEq)]
struct SelectionReport {
    functions: usize,
    ordinary_units: usize,
    candidates: BTreeMap<SelectionCandidate, usize>,
    rejection_reasons: BTreeMap<OrdinaryReason, usize>,
}

#[cfg(test)]
fn selection_report(plan: &SelectedProgramPlan) -> SelectionReport {
    let mut ordinary_units = 0;
    let mut candidates = BTreeMap::new();
    let mut rejection_reasons = BTreeMap::new();
    for unit in plan
        .functions
        .values()
        .flat_map(|function| function.units.iter())
    {
        match unit {
            SelectedUnit::Ordinary(range) => {
                ordinary_units += 1;
                *rejection_reasons.entry(range.reason).or_default() += 1;
            }
            SelectedUnit::Superinstruction(_) => {
                *candidates
                    .entry(SelectionCandidate::I64CompareImmediateBranch)
                    .or_default() += 1;
            }
            SelectedUnit::ScalarBlock(_) => {
                *candidates
                    .entry(SelectionCandidate::ScalarBlock)
                    .or_default() += 1;
            }
        }
    }
    SelectionReport {
        functions: plan.functions.len(),
        ordinary_units,
        candidates,
        rejection_reasons,
    }
}

#[cfg(test)]
mod tests;
