use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{MirBlockId, MirStatementId, MirTerminatorKind};

use super::{FunctionVerifier, MirVerifyError, MirVerifyErrorKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StatementLocation {
    pub(crate) block: MirBlockId,
    pub(crate) index: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct FunctionGraph {
    blocks: BTreeSet<MirBlockId>,
    successors: BTreeMap<MirBlockId, Vec<MirBlockId>>,
    predecessors: BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
    statement_locations: BTreeMap<MirStatementId, StatementLocation>,
    dominators: BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
}

impl FunctionGraph {
    pub(crate) fn blocks(&self) -> impl Iterator<Item = MirBlockId> + '_ {
        self.blocks.iter().copied()
    }

    pub(crate) fn predecessors(&self, block: MirBlockId) -> impl Iterator<Item = MirBlockId> + '_ {
        self.predecessors
            .get(&block)
            .into_iter()
            .flat_map(|values| values.iter().copied())
    }

    pub(crate) fn statement_location(
        &self,
        statement: MirStatementId,
    ) -> Option<StatementLocation> {
        self.statement_locations.get(&statement).copied()
    }

    pub(crate) fn dominates(&self, dominator: MirBlockId, block: MirBlockId) -> bool {
        self.dominators
            .get(&block)
            .is_some_and(|values| values.contains(&dominator))
    }

    pub(crate) fn successors(&self, block: MirBlockId) -> impl Iterator<Item = MirBlockId> + '_ {
        self.successors
            .get(&block)
            .into_iter()
            .flat_map(|values| values.iter().copied())
    }
}

pub(crate) fn analyze(verifier: &FunctionVerifier<'_>) -> Result<FunctionGraph, MirVerifyError> {
    let function = verifier.function;
    let entry = function.entry_block();
    if function.block(entry).is_none() {
        return Err(verifier.error(
            None,
            None,
            function.origin(),
            MirVerifyErrorKind::MissingBlock(entry),
        ));
    }

    let blocks = function.blocks().map(|(id, _)| id).collect::<BTreeSet<_>>();
    let mut successors = BTreeMap::new();
    let mut predecessors = blocks
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut statement_locations = BTreeMap::new();

    for (block_id, block) in function.blocks() {
        let terminator = block.terminator().ok_or_else(|| {
            verifier.error(
                Some(block_id),
                None,
                function.origin(),
                MirVerifyErrorKind::UnterminatedBlock,
            )
        })?;

        for (index, statement_id) in block.statements().iter().copied().enumerate() {
            let statement = function.statement(statement_id).ok_or_else(|| {
                verifier.error(
                    Some(block_id),
                    Some(statement_id),
                    terminator.origin,
                    MirVerifyErrorKind::MissingStatement(statement_id),
                )
            })?;
            if statement_locations
                .insert(
                    statement_id,
                    StatementLocation {
                        block: block_id,
                        index,
                    },
                )
                .is_some()
            {
                return Err(verifier.error(
                    Some(block_id),
                    Some(statement_id),
                    statement.origin,
                    MirVerifyErrorKind::DuplicateStatementPlacement(statement_id),
                ));
            }
        }

        let block_successors = terminator_successors(&terminator.kind);
        for target in &block_successors {
            if !blocks.contains(target) {
                return Err(verifier.error(
                    Some(block_id),
                    None,
                    terminator.origin,
                    MirVerifyErrorKind::MissingBlock(*target),
                ));
            }
            predecessors
                .get_mut(target)
                .expect("verified successor has a predecessor entry")
                .insert(block_id);
        }
        successors.insert(block_id, block_successors);
    }

    for (statement_id, statement) in function.statements() {
        if !statement_locations.contains_key(&statement_id) {
            return Err(verifier.error(
                None,
                Some(statement_id),
                statement.origin,
                MirVerifyErrorKind::OrphanStatement(statement_id),
            ));
        }
    }

    let reachable = reachable_blocks(entry, &successors);
    if let Some(block) = blocks.iter().copied().find(|block| {
        if reachable.contains(block) {
            return false;
        }
        !matches!(
            function.block(*block).and_then(|block| block.terminator()),
            Some(terminator) if matches!(terminator.kind, MirTerminatorKind::Unreachable)
        )
    }) {
        let origin = function
            .block(block)
            .and_then(|block| block.terminator())
            .map_or(function.origin(), |terminator| terminator.origin);
        return Err(verifier.error(
            Some(block),
            None,
            origin,
            MirVerifyErrorKind::UnreachableBlock,
        ));
    }

    let dominators = compute_dominators(entry, &blocks, &predecessors);
    Ok(FunctionGraph {
        blocks,
        successors,
        predecessors,
        statement_locations,
        dominators,
    })
}

fn terminator_successors(kind: &MirTerminatorKind) -> Vec<MirBlockId> {
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
            ..
        } => continuations
            .iter()
            .map(|continuation| continuation.block)
            .chain([*propagate, *invalid])
            .collect(),
        MirTerminatorKind::IteratorNext { next, done, .. }
        | MirTerminatorKind::RangeNext { next, done, .. } => vec![*next, *done],
        MirTerminatorKind::Return(_)
        | MirTerminatorKind::TryTypeMismatch { .. }
        | MirTerminatorKind::Unreachable => Vec::new(),
    }
}

fn reachable_blocks(
    entry: MirBlockId,
    successors: &BTreeMap<MirBlockId, Vec<MirBlockId>>,
) -> BTreeSet<MirBlockId> {
    let mut reachable = BTreeSet::new();
    let mut pending = VecDeque::from([entry]);
    while let Some(block) = pending.pop_front() {
        if !reachable.insert(block) {
            continue;
        }
        if let Some(targets) = successors.get(&block) {
            pending.extend(targets.iter().copied());
        }
    }
    reachable
}

fn compute_dominators(
    entry: MirBlockId,
    blocks: &BTreeSet<MirBlockId>,
    predecessors: &BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
) -> BTreeMap<MirBlockId, BTreeSet<MirBlockId>> {
    let mut dominators = blocks
        .iter()
        .copied()
        .map(|block| {
            let initial = if block == entry {
                BTreeSet::from([entry])
            } else {
                blocks.clone()
            };
            (block, initial)
        })
        .collect::<BTreeMap<_, _>>();

    loop {
        let mut changed = false;
        for block in blocks.iter().copied().filter(|block| *block != entry) {
            let mut incoming = predecessors
                .get(&block)
                .into_iter()
                .flat_map(|values| values.iter())
                .map(|predecessor| dominators[predecessor].clone());
            let mut next = incoming.next().unwrap_or_default();
            for values in incoming {
                next.retain(|value| values.contains(value));
            }
            next.insert(block);
            if dominators[&block] != next {
                dominators.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return dominators;
        }
    }
}
