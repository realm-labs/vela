use std::collections::{BTreeMap, BTreeSet};

use crate::verifier::dataflow::{visit_statement_operands, visit_terminator_operands};
use crate::{
    MirBlockId, MirDebugLocalId, MirFunction, MirLiveRegion, MirLiveValue, MirLiveness, MirOperand,
    MirPlace, MirSafepointId, MirTerminatorKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LivenessAnalysis {
    pub(crate) liveness: MirLiveness,
    pub(crate) safepoints: BTreeMap<MirSafepointId, BTreeSet<MirLiveValue>>,
    pub(crate) debug_regions: BTreeMap<MirDebugLocalId, MirLiveRegion>,
}

pub(crate) fn apply(function: &mut MirFunction) {
    let analysis = analyze(function);
    function.apply_live_metadata(&analysis.safepoints, &analysis.debug_regions);
    function.set_liveness(analysis.liveness);
}

pub(crate) fn analyze(function: &MirFunction) -> LivenessAnalysis {
    let blocks = function
        .blocks()
        .map(|(block, _)| block)
        .collect::<Vec<_>>();
    let mut live_in = blocks
        .iter()
        .map(|block| (*block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut live_out = live_in.clone();

    loop {
        let mut changed = false;
        for block in blocks.iter().rev().copied() {
            let (next_in, next_out, _) = block_sets(function, block, &live_in);
            changed |= live_in.get(&block) != Some(&next_in);
            changed |= live_out.get(&block) != Some(&next_out);
            live_in.insert(block, next_in);
            live_out.insert(block, next_out);
        }
        if !changed {
            break;
        }
    }

    let mut statement_live_before = BTreeMap::new();
    let mut statement_live_after = BTreeMap::new();
    let mut terminator_live_before = BTreeMap::new();
    for block in blocks.iter().copied() {
        let (_, _, terminator_before) = block_sets(function, block, &live_in);
        terminator_live_before.insert(block, terminator_before.clone());
        let data = function.block(block).expect("liveness block exists");
        let mut current = terminator_before;
        for statement in data.statements().iter().rev().copied() {
            statement_live_after.insert(statement, current.clone());
            let record = function
                .statement(statement)
                .expect("liveness statement exists");
            if let Some(destination) = record.destination {
                current.remove(&place_value(destination));
            }
            current.extend(statement_uses(&record.kind));
            statement_live_before.insert(statement, current.clone());
        }
    }

    let mut liveness = MirLiveness {
        block_live_in: live_in,
        block_live_out: live_out,
        statement_live_before,
        statement_live_after,
        ..MirLiveness::default()
    };
    liveness.mark_computed();
    let safepoints = safepoint_sets(function, &liveness, &terminator_live_before);
    let debug_regions = debug_regions(function, &liveness);
    LivenessAnalysis {
        liveness,
        safepoints,
        debug_regions,
    }
}

pub(crate) fn sealed_analyses(function: &MirFunction) -> crate::MirFunctionAnalyses {
    let analysis = analyze(function);
    let live_before_safepoint = analysis
        .safepoints
        .iter()
        .map(|(safepoint, values)| {
            let roots = values
                .iter()
                .copied()
                .filter(|value| is_possible_root(function, *value))
                .collect();
            (*safepoint, roots)
        })
        .collect();
    let debug_availability = lexical_debug_availability(function);
    crate::MirFunctionAnalyses {
        value_liveness: analysis.liveness,
        root_liveness: crate::MirRootLiveness {
            live_before_safepoint,
        },
        debug_availability,
        facts: crate::MirProgramPointFacts::default(),
    }
}

fn lexical_debug_availability(function: &MirFunction) -> crate::MirDebugAvailability {
    let blocks = function
        .blocks()
        .map(|(block, _)| block)
        .collect::<Vec<_>>();
    let entry = function.entry_block();
    let initial = function
        .debug_locals()
        .filter_map(|(id, debug)| {
            matches!(
                debug.kind,
                crate::DebugLocalKind::Parameter | crate::DebugLocalKind::Capture
            )
            .then_some(id)
        })
        .collect::<BTreeSet<_>>();
    let mut block_in = blocks
        .iter()
        .map(|block| (*block, (*block == entry).then_some(initial.clone())))
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in blocks.iter().copied().filter(|block| *block != entry) {
            let incoming = function.blocks().filter_map(|(predecessor, data)| {
                let mut state = block_in.get(&predecessor)?.clone()?;
                if !successors(&data.terminator()?.kind).contains(&block) {
                    return None;
                }
                for statement in data.statements() {
                    let destination = function.statement(*statement)?.destination;
                    for (id, debug) in function.debug_locals() {
                        if destination == Some(MirPlace::Local(debug.storage)) {
                            state.insert(id);
                        }
                    }
                }
                Some(state)
            });
            let mut incoming = incoming.peekable();
            let next = incoming.next().map(|mut values| {
                for other in incoming {
                    values.retain(|value| other.contains(value));
                }
                values
            });
            if block_in.get(&block) != Some(&next) {
                block_in.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut statement_before = BTreeMap::new();
    let mut regions = function
        .debug_locals()
        .map(|(id, _)| (id, MirLiveRegion::default()))
        .collect::<BTreeMap<_, _>>();
    for block in blocks {
        let Some(mut state) = block_in.get(&block).cloned().flatten() else {
            continue;
        };
        for id in &state {
            regions.entry(*id).or_default().blocks.insert(block);
        }
        let data = function
            .block(block)
            .expect("debug availability block exists");
        for statement in data.statements() {
            statement_before.insert(*statement, state.clone());
            let destination = function
                .statement(*statement)
                .expect("debug availability statement exists")
                .destination;
            for (id, debug) in function.debug_locals() {
                if destination == Some(MirPlace::Local(debug.storage)) {
                    state.insert(id);
                    regions.entry(id).or_default().blocks.insert(block);
                }
            }
        }
    }
    crate::MirDebugAvailability {
        locals: regions,
        statement_before,
    }
}

fn is_possible_root(function: &MirFunction, value: MirLiveValue) -> bool {
    let value_type = match value {
        MirLiveValue::Local(local) => function.local(local).map(|local| local.value_type),
        MirLiveValue::Temp(temp) => function.temp(temp).map(|temp| temp.value_type),
    };
    match value_type {
        Some(crate::MirValueType::Unit | crate::MirValueType::Range) | None => false,
        Some(crate::MirValueType::Primitive(tag)) => matches!(
            tag,
            vela_common::PrimitiveTag::String | vela_common::PrimitiveTag::Bytes
        ),
        Some(_) => true,
    }
}

fn block_sets(
    function: &MirFunction,
    block: MirBlockId,
    live_in: &BTreeMap<MirBlockId, BTreeSet<MirLiveValue>>,
) -> (
    BTreeSet<MirLiveValue>,
    BTreeSet<MirLiveValue>,
    BTreeSet<MirLiveValue>,
) {
    let data = function.block(block).expect("liveness block exists");
    let terminator = data.terminator().expect("liveness block is terminated");
    let mut out = BTreeSet::new();
    let mut before_terminator = terminator_uses(&terminator.kind);
    for successor in successors(&terminator.kind) {
        let successor_live = live_in.get(&successor).cloned().unwrap_or_default();
        out.extend(successor_live.iter().copied());
        let mut edge_live = successor_live;
        for definition in edge_definitions(&terminator.kind, successor) {
            edge_live.remove(&definition);
        }
        before_terminator.extend(edge_live);
    }
    let mut input = before_terminator.clone();
    for statement in data.statements().iter().rev().copied() {
        let record = function
            .statement(statement)
            .expect("liveness statement exists");
        if let Some(destination) = record.destination {
            input.remove(&place_value(destination));
        }
        input.extend(statement_uses(&record.kind));
    }
    (input, out, before_terminator)
}

fn statement_uses(kind: &crate::MirStatementKind) -> BTreeSet<MirLiveValue> {
    let mut uses = BTreeSet::new();
    visit_statement_operands(kind, |operand| {
        if let Some(value) = operand_value(operand) {
            uses.insert(value);
        }
        Ok(())
    })
    .expect("operand traversal is infallible for liveness");
    uses
}

fn terminator_uses(kind: &MirTerminatorKind) -> BTreeSet<MirLiveValue> {
    let mut uses = BTreeSet::new();
    visit_terminator_operands(kind, |operand| {
        if let Some(value) = operand_value(operand) {
            uses.insert(value);
        }
        Ok(())
    })
    .expect("operand traversal is infallible for liveness");
    if let MirTerminatorKind::RangeNext {
        cursor, exhausted, ..
    } = kind
    {
        uses.insert(MirLiveValue::Local(*cursor));
        uses.insert(MirLiveValue::Local(*exhausted));
    }
    uses
}

fn successors(kind: &MirTerminatorKind) -> Vec<MirBlockId> {
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

fn edge_definitions(kind: &MirTerminatorKind, successor: MirBlockId) -> BTreeSet<MirLiveValue> {
    let mut definitions = BTreeSet::new();
    match kind {
        MirTerminatorKind::IteratorNext { item, next, .. } if *next == successor => {
            definitions.insert(MirLiveValue::Local(*item));
        }
        MirTerminatorKind::RangeNext {
            cursor,
            exhausted,
            item,
            next,
            done,
            ..
        } if *next == successor || *done == successor => {
            definitions.insert(MirLiveValue::Local(*cursor));
            definitions.insert(MirLiveValue::Local(*exhausted));
            if *next == successor {
                definitions.insert(MirLiveValue::Local(*item));
            }
        }
        _ => {}
    }
    definitions
}

fn safepoint_sets(
    function: &MirFunction,
    liveness: &MirLiveness,
    terminator_live_before: &BTreeMap<MirBlockId, BTreeSet<MirLiveValue>>,
) -> BTreeMap<MirSafepointId, BTreeSet<MirLiveValue>> {
    let mut sets = BTreeMap::new();
    for (block, data) in function.blocks() {
        for statement in data.statements() {
            let record = function
                .statement(*statement)
                .expect("liveness statement exists");
            if let Some(safepoint) = record.safepoint {
                sets.insert(
                    safepoint,
                    liveness
                        .statement_live_before
                        .get(statement)
                        .cloned()
                        .unwrap_or_default(),
                );
            }
        }
        if let Some(terminator) = data.terminator()
            && let Some(safepoint) = terminator.safepoint
        {
            sets.insert(
                safepoint,
                terminator_live_before
                    .get(&block)
                    .cloned()
                    .unwrap_or_default(),
            );
        }
    }
    sets
}

fn debug_regions(
    function: &MirFunction,
    liveness: &MirLiveness,
) -> BTreeMap<MirDebugLocalId, MirLiveRegion> {
    let entry = function.entry_block();
    function
        .debug_locals()
        .map(|(id, debug)| {
            let value = MirLiveValue::Local(debug.storage);
            let mut blocks = function
                .blocks()
                .filter_map(|(block, data)| {
                    let live = liveness
                        .block_live_in
                        .get(&block)
                        .is_some_and(|values| values.contains(&value))
                        || liveness
                            .block_live_out
                            .get(&block)
                            .is_some_and(|values| values.contains(&value))
                        || data.statements().iter().any(|statement| {
                            function.statement(*statement).is_some_and(|statement| {
                                statement.destination == Some(MirPlace::Local(debug.storage))
                            })
                        });
                    live.then_some(block)
                })
                .collect::<BTreeSet<_>>();
            if function
                .parameters()
                .iter()
                .any(|parameter| parameter.storage == debug.storage)
                || function
                    .captures()
                    .iter()
                    .any(|capture| capture.storage == debug.storage)
            {
                blocks.insert(entry);
            }
            (id, MirLiveRegion { blocks })
        })
        .collect()
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
