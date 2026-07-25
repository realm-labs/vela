use std::collections::{BTreeMap, BTreeSet};

use crate::verifier::dataflow::{visit_statement_operands, visit_terminator_operands};
use crate::{
    MirBlockId, MirCall, MirFunction, MirHostOperation, MirLiveValue, MirLiveness, MirOperand,
    MirPlace, MirRvalue, MirStateOperation, MirStatementId, MirStatementKind, MirTerminatorKind,
};

/// Conservative release points for values proven to originate from a scoped
/// borrowed-host return.
///
/// This is derived after MIR verification. It is deliberately absent for
/// ordinary host references, aliased borrowed values, container/closure
/// escapes, returned values, and explicit `host::release` calls.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirBorrowReleaseSchedule {
    after_statement: BTreeMap<MirStatementId, Vec<MirLiveValue>>,
    edges: BTreeMap<(MirBlockId, MirBlockId), Vec<MirLiveValue>>,
}

impl MirBorrowReleaseSchedule {
    #[must_use]
    pub fn after_statement(&self, statement: MirStatementId) -> &[MirLiveValue] {
        self.after_statement
            .get(&statement)
            .map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn edge(&self, from: MirBlockId, to: MirBlockId) -> &[MirLiveValue] {
        self.edges.get(&(from, to)).map_or(&[], Vec::as_slice)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum UseLocation {
    Statement(MirStatementId),
    Terminator(MirBlockId),
}

pub(crate) fn analyze(function: &MirFunction, liveness: &MirLiveness) -> MirBorrowReleaseSchedule {
    let statement_blocks = statement_blocks(function);
    let uses = collect_uses(function);
    let dominators = dominators(function);
    let mut after_statement = BTreeMap::<MirStatementId, BTreeSet<MirLiveValue>>::new();
    let mut edges = BTreeMap::<(MirBlockId, MirBlockId), BTreeSet<MirLiveValue>>::new();

    for (origin, statement) in function.statements() {
        if !is_direct_scoped_return(&statement.kind) {
            continue;
        }
        let Some(destination) = statement.destination else {
            continue;
        };
        let origin_block = statement_blocks[&origin];
        let (candidate, definition) = follow_linear_copies(
            function,
            liveness,
            &uses,
            &statement_blocks,
            place_value(destination),
            origin,
            origin_block,
        );
        let tracked = track_ephemeral_temp_aliases(function, &uses, candidate);
        let candidate_uses = tracked
            .iter()
            .flat_map(|value| uses.get(value).into_iter().flatten().copied())
            .collect::<BTreeSet<_>>();
        if tracked.iter().any(|value| {
            uses.get(value).is_some_and(|locations| {
                locations
                    .iter()
                    .any(|location| is_manual_release_or_escape(function, *value, *location))
            })
        }) {
            continue;
        }

        if candidate_uses.is_empty() {
            if !liveness
                .statement_live_after
                .get(&definition)
                .is_some_and(|values| tracked.iter().any(|value| values.contains(value)))
            {
                after_statement
                    .entry(definition)
                    .or_default()
                    .insert(candidate);
            }
            continue;
        }

        for location in &candidate_uses {
            let UseLocation::Statement(statement) = *location else {
                continue;
            };
            if !liveness
                .statement_live_after
                .get(&statement)
                .is_some_and(|values| tracked.iter().any(|value| values.contains(value)))
            {
                let release = operands_in_statement(
                    &function
                        .statement(statement)
                        .expect("borrow-release statement exists")
                        .kind,
                )
                .into_iter()
                .find(|value| tracked.contains(value))
                .unwrap_or(candidate);
                after_statement
                    .entry(statement)
                    .or_default()
                    .insert(release);
            }
        }

        for (block, data) in function.blocks() {
            if !dominators
                .get(&block)
                .is_some_and(|values| values.contains(&origin_block))
            {
                continue;
            }
            let Some(terminator) = data.terminator() else {
                continue;
            };
            let terminator_uses = operands_in_terminator(&terminator.kind);
            let active = tracked.iter().copied().find(|candidate| {
                liveness
                    .block_live_out
                    .get(&block)
                    .is_some_and(|values| values.contains(candidate))
                    || terminator_uses.contains(candidate)
            });
            let Some(active) = active else {
                continue;
            };
            for successor in successors(&terminator.kind) {
                if !liveness
                    .block_live_in
                    .get(&successor)
                    .is_some_and(|values| tracked.iter().any(|value| values.contains(value)))
                {
                    edges.entry((block, successor)).or_default().insert(active);
                }
            }
        }
    }

    MirBorrowReleaseSchedule {
        after_statement: after_statement
            .into_iter()
            .map(|(site, values)| (site, values.into_iter().collect()))
            .collect(),
        edges: edges
            .into_iter()
            .map(|(site, values)| (site, values.into_iter().collect()))
            .collect(),
    }
}

fn is_direct_scoped_return(kind: &MirStatementKind) -> bool {
    match kind {
        MirStatementKind::Call(MirCall::NativeFunction {
            scoped_borrow_return: true,
            ..
        }) => true,
        MirStatementKind::Host(MirHostOperation::Call { target, .. }) => {
            target.scoped_borrow_return
        }
        _ => false,
    }
}

fn follow_linear_copies(
    function: &MirFunction,
    liveness: &MirLiveness,
    uses: &BTreeMap<MirLiveValue, Vec<UseLocation>>,
    statement_blocks: &BTreeMap<MirStatementId, MirBlockId>,
    mut value: MirLiveValue,
    mut definition: MirStatementId,
    origin_block: MirBlockId,
) -> (MirLiveValue, MirStatementId) {
    let mut visited = BTreeSet::new();
    while visited.insert(value) {
        if matches!(value, MirLiveValue::Local(_)) {
            break;
        }
        let Some([UseLocation::Statement(statement_id)]) = uses.get(&value).map(Vec::as_slice)
        else {
            break;
        };
        if statement_blocks.get(statement_id) != Some(&origin_block)
            || liveness
                .statement_live_after
                .get(statement_id)
                .is_some_and(|values| values.contains(&value))
        {
            break;
        }
        let Some(statement) = function.statement(*statement_id) else {
            break;
        };
        let MirStatementKind::Assign(MirRvalue::Use(source)) = &statement.kind else {
            break;
        };
        if operand_value(source) != Some(value) {
            break;
        }
        let Some(destination) = statement.destination else {
            break;
        };
        value = place_value(destination);
        definition = *statement_id;
    }
    (value, definition)
}

fn track_ephemeral_temp_aliases(
    function: &MirFunction,
    uses: &BTreeMap<MirLiveValue, Vec<UseLocation>>,
    candidate: MirLiveValue,
) -> BTreeSet<MirLiveValue> {
    let mut tracked = BTreeSet::from([candidate]);
    loop {
        let mut changed = false;
        for value in tracked.clone() {
            for location in uses.get(&value).into_iter().flatten() {
                let UseLocation::Statement(statement) = *location else {
                    continue;
                };
                let Some(statement) = function.statement(statement) else {
                    continue;
                };
                if matches!(
                    &statement.kind,
                    MirStatementKind::Assign(MirRvalue::Use(source))
                        if operand_value(source) == Some(value)
                ) && let Some(MirPlace::Temp(temp)) = statement.destination
                {
                    changed |= tracked.insert(MirLiveValue::Temp(temp));
                }
            }
        }
        if !changed {
            return tracked;
        }
    }
}

fn is_manual_release_or_escape(
    function: &MirFunction,
    candidate: MirLiveValue,
    location: UseLocation,
) -> bool {
    match location {
        UseLocation::Statement(statement) => {
            let Some(statement) = function.statement(statement) else {
                return true;
            };
            match &statement.kind {
                MirStatementKind::Host(MirHostOperation::ReleaseBorrowLease { root })
                    if operand_value(root) == Some(candidate) =>
                {
                    true
                }
                kind if is_direct_scoped_return(kind)
                    && operands_in_statement(kind).contains(&candidate) =>
                {
                    true
                }
                MirStatementKind::Assign(MirRvalue::Use(source))
                    if operand_value(source) == Some(candidate)
                        && matches!(statement.destination, Some(MirPlace::Local(_))) =>
                {
                    true
                }
                MirStatementKind::Allocate(_)
                | MirStatementKind::WriteField { .. }
                | MirStatementKind::Reflect(_) => true,
                MirStatementKind::State(MirStateOperation::WriteVmState { value, .. })
                    if operand_value(value) == Some(candidate) =>
                {
                    true
                }
                MirStatementKind::Index(crate::MirIndexOperation::Write { value, .. })
                    if operand_value(value) == Some(candidate) =>
                {
                    true
                }
                _ => false,
            }
        }
        UseLocation::Terminator(block) => function
            .block(block)
            .and_then(|block| block.terminator())
            .is_none_or(|terminator| {
                matches!(
                    &terminator.kind,
                    MirTerminatorKind::Return(Some(value)) if operand_value(value) == Some(candidate)
                )
            }),
    }
}

fn collect_uses(function: &MirFunction) -> BTreeMap<MirLiveValue, Vec<UseLocation>> {
    let mut uses = BTreeMap::<MirLiveValue, Vec<UseLocation>>::new();
    for (statement_id, statement) in function.statements() {
        let location = UseLocation::Statement(statement_id);
        visit_statement_operands(&statement.kind, |operand| {
            if let Some(value) = operand_value(operand) {
                let locations = uses.entry(value).or_default();
                if !locations.contains(&location) {
                    locations.push(location);
                }
            }
            Ok(())
        })
        .expect("borrow-release operand traversal is infallible");
    }
    for (block_id, block) in function.blocks() {
        let Some(terminator) = block.terminator() else {
            continue;
        };
        let location = UseLocation::Terminator(block_id);
        visit_terminator_operands(&terminator.kind, |operand| {
            if let Some(value) = operand_value(operand) {
                let locations = uses.entry(value).or_default();
                if !locations.contains(&location) {
                    locations.push(location);
                }
            }
            Ok(())
        })
        .expect("borrow-release operand traversal is infallible");
    }
    uses
}

fn operands_in_terminator(kind: &MirTerminatorKind) -> BTreeSet<MirLiveValue> {
    let mut values = BTreeSet::new();
    visit_terminator_operands(kind, |operand| {
        if let Some(value) = operand_value(operand) {
            values.insert(value);
        }
        Ok(())
    })
    .expect("borrow-release operand traversal is infallible");
    values
}

fn operands_in_statement(kind: &MirStatementKind) -> BTreeSet<MirLiveValue> {
    let mut values = BTreeSet::new();
    visit_statement_operands(kind, |operand| {
        if let Some(value) = operand_value(operand) {
            values.insert(value);
        }
        Ok(())
    })
    .expect("borrow-release operand traversal is infallible");
    values
}

fn statement_blocks(function: &MirFunction) -> BTreeMap<MirStatementId, MirBlockId> {
    function
        .blocks()
        .flat_map(|(block, data)| {
            data.statements()
                .iter()
                .copied()
                .map(move |statement| (statement, block))
        })
        .collect()
}

fn dominators(function: &MirFunction) -> BTreeMap<MirBlockId, BTreeSet<MirBlockId>> {
    let blocks = function.blocks().map(|(id, _)| id).collect::<Vec<_>>();
    let all = blocks.iter().copied().collect::<BTreeSet<_>>();
    let entry = function.entry_block();
    let mut predecessors = blocks
        .iter()
        .copied()
        .map(|block| (block, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for (block, data) in function.blocks() {
        if let Some(terminator) = data.terminator() {
            for successor in successors(&terminator.kind) {
                predecessors.entry(successor).or_default().push(block);
            }
        }
    }
    let mut values = blocks
        .iter()
        .copied()
        .map(|block| {
            (
                block,
                if block == entry {
                    BTreeSet::from([entry])
                } else {
                    all.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in blocks.iter().copied().filter(|block| *block != entry) {
            let mut next = predecessors[&block]
                .iter()
                .map(|predecessor| values[predecessor].clone())
                .reduce(|left, right| left.intersection(&right).copied().collect())
                .unwrap_or_default();
            next.insert(block);
            if values.get(&block) != Some(&next) {
                values.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return values;
        }
    }
}

fn successors(kind: &MirTerminatorKind) -> Vec<MirBlockId> {
    match kind {
        MirTerminatorKind::AwaitCall { resume, .. } | MirTerminatorKind::Jump(resume) => {
            vec![*resume]
        }
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
        MirTerminatorKind::Return(_)
        | MirTerminatorKind::TryTypeMismatch { .. }
        | MirTerminatorKind::Unreachable => Vec::new(),
    }
}

const fn place_value(place: MirPlace) -> MirLiveValue {
    match place {
        MirPlace::Local(local) => MirLiveValue::Local(local),
        MirPlace::Temp(temp) => MirLiveValue::Temp(temp),
    }
}

const fn operand_value(operand: &MirOperand) -> Option<MirLiveValue> {
    match operand {
        MirOperand::Immediate(_) => None,
        MirOperand::Local(local) => Some(MirLiveValue::Local(*local)),
        MirOperand::Temp(temp) => Some(MirLiveValue::Temp(*temp)),
    }
}
