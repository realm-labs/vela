use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

use vela_bytecode::{
    DebugNameId, ExecutableGenerationId, InstructionOffset, ProfileLayout, ScalarBlockPlanId,
    ScalarSourcePointId, ScriptFunctionHandle, SelectedPhysicalUnitKind,
};
use vela_vm::{ScalarLoopProfileEvent, VmBytecodeProfiler};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BytecodeProfileSnapshot {
    generation: ExecutableGenerationId,
    functions: Vec<FunctionBytecodeProfile>,
}

impl BytecodeProfileSnapshot {
    #[must_use]
    pub const fn generation(&self) -> ExecutableGenerationId {
        self.generation
    }

    #[must_use]
    pub fn functions(&self) -> &[FunctionBytecodeProfile] {
        &self.functions
    }

    #[must_use]
    pub fn summary(&self) -> BytecodeProfileSummary {
        self.functions.iter().fold(
            BytecodeProfileSummary::default(),
            |mut summary, function| {
                summary.ordinary_instruction_hits = summary
                    .ordinary_instruction_hits
                    .saturating_add(function.ordinary_instruction_hits);
                for unit in &function.superinstructions {
                    summary.superinstruction_hits =
                        summary.superinstruction_hits.saturating_add(unit.hits);
                    summary.eliminated_dispatches = summary
                        .eliminated_dispatches
                        .saturating_add(unit.eliminated_dispatches);
                }
                for unit in &function.scalar_units {
                    summary.scalar_block_entries =
                        summary.scalar_block_entries.saturating_add(unit.entry_hits);
                    summary.scalar_compact_operation_hits = summary
                        .scalar_compact_operation_hits
                        .saturating_add(unit.compact_operation_hits);
                    if let Some(loop_profile) = unit.loop_profile {
                        summary.scalar_loop_entries = summary
                            .scalar_loop_entries
                            .saturating_add(loop_profile.entries);
                        summary.scalar_loop_iterations = summary
                            .scalar_loop_iterations
                            .saturating_add(loop_profile.iterations);
                        summary.scalar_loop_exits =
                            summary.scalar_loop_exits.saturating_add(loop_profile.exits);
                        summary.scalar_loop_charged_backedges = summary
                            .scalar_loop_charged_backedges
                            .saturating_add(loop_profile.charged_backedges);
                    }
                }
                summary
            },
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BytecodeProfileSummary {
    ordinary_instruction_hits: u64,
    superinstruction_hits: u64,
    eliminated_dispatches: u64,
    scalar_block_entries: u64,
    scalar_compact_operation_hits: u64,
    scalar_loop_entries: u64,
    scalar_loop_iterations: u64,
    scalar_loop_exits: u64,
    scalar_loop_charged_backedges: u64,
}

impl BytecodeProfileSummary {
    #[must_use]
    pub const fn ordinary_instruction_hits(self) -> u64 {
        self.ordinary_instruction_hits
    }

    #[must_use]
    pub const fn superinstruction_hits(self) -> u64 {
        self.superinstruction_hits
    }

    #[must_use]
    pub const fn eliminated_dispatches(self) -> u64 {
        self.eliminated_dispatches
    }

    #[must_use]
    pub const fn scalar_block_entries(self) -> u64 {
        self.scalar_block_entries
    }

    #[must_use]
    pub const fn scalar_compact_operation_hits(self) -> u64 {
        self.scalar_compact_operation_hits
    }

    #[must_use]
    pub const fn scalar_loop_entries(self) -> u64 {
        self.scalar_loop_entries
    }

    #[must_use]
    pub const fn scalar_loop_iterations(self) -> u64 {
        self.scalar_loop_iterations
    }

    #[must_use]
    pub const fn scalar_loop_exits(self) -> u64 {
        self.scalar_loop_exits
    }

    #[must_use]
    pub const fn scalar_loop_charged_backedges(self) -> u64 {
        self.scalar_loop_charged_backedges
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionBytecodeProfile {
    handle: ScriptFunctionHandle,
    debug_name: DebugNameId,
    instruction_hits: Vec<u64>,
    ordinary_instruction_hits: u64,
    superinstructions: Vec<SuperinstructionBytecodeProfile>,
    scalar_units: Vec<ScalarUnitBytecodeProfile>,
}

impl FunctionBytecodeProfile {
    #[must_use]
    pub const fn handle(&self) -> ScriptFunctionHandle {
        self.handle
    }

    #[must_use]
    pub const fn debug_name(&self) -> DebugNameId {
        self.debug_name
    }

    #[must_use]
    pub fn instruction_hits(&self) -> &[u64] {
        &self.instruction_hits
    }

    #[must_use]
    pub const fn ordinary_instruction_hits(&self) -> u64 {
        self.ordinary_instruction_hits
    }

    #[must_use]
    pub fn superinstructions(&self) -> &[SuperinstructionBytecodeProfile] {
        &self.superinstructions
    }

    #[must_use]
    pub fn scalar_units(&self) -> &[ScalarUnitBytecodeProfile] {
        &self.scalar_units
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SuperinstructionBytecodeProfile {
    offset: InstructionOffset,
    kind: SelectedPhysicalUnitKind,
    hits: u64,
    eliminated_dispatches: u64,
}

impl SuperinstructionBytecodeProfile {
    #[must_use]
    pub const fn offset(self) -> InstructionOffset {
        self.offset
    }

    #[must_use]
    pub const fn kind(self) -> SelectedPhysicalUnitKind {
        self.kind
    }

    #[must_use]
    pub const fn hits(self) -> u64 {
        self.hits
    }

    #[must_use]
    pub const fn eliminated_dispatches(self) -> u64 {
        self.eliminated_dispatches
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarUnitBytecodeProfile {
    offset: InstructionOffset,
    plan: ScalarBlockPlanId,
    entry_hits: u64,
    compact_operation_hits: u64,
    subpoint_hits: Vec<u64>,
    loop_profile: Option<ScalarLoopBytecodeProfile>,
}

impl ScalarUnitBytecodeProfile {
    #[must_use]
    pub const fn offset(&self) -> InstructionOffset {
        self.offset
    }

    #[must_use]
    pub const fn plan(&self) -> ScalarBlockPlanId {
        self.plan
    }

    #[must_use]
    pub const fn entry_hits(&self) -> u64 {
        self.entry_hits
    }

    #[must_use]
    pub const fn compact_operation_hits(&self) -> u64 {
        self.compact_operation_hits
    }

    #[must_use]
    pub fn subpoint_hits(&self) -> &[u64] {
        &self.subpoint_hits
    }

    #[must_use]
    pub const fn loop_profile(&self) -> Option<&ScalarLoopBytecodeProfile> {
        self.loop_profile.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarLoopBytecodeProfile {
    entries: u64,
    iterations: u64,
    exits: u64,
    charged_backedges: u64,
}

impl ScalarLoopBytecodeProfile {
    #[must_use]
    pub const fn entries(self) -> u64 {
        self.entries
    }

    #[must_use]
    pub const fn iterations(self) -> u64 {
        self.iterations
    }

    #[must_use]
    pub const fn exits(self) -> u64 {
        self.exits
    }

    #[must_use]
    pub const fn charged_backedges(self) -> u64 {
        self.charged_backedges
    }
}

#[derive(Debug)]
pub(crate) struct GenerationBytecodeProfile {
    functions: Box<[FunctionCounters]>,
    function_index: BTreeMap<DebugNameId, usize>,
}

#[derive(Debug)]
struct FunctionCounters {
    handle: ScriptFunctionHandle,
    debug_name: DebugNameId,
    instruction_hits: Box<[AtomicU64]>,
    ordinary_offsets: Box<[InstructionOffset]>,
    superinstructions: Box<[SuperinstructionCounters]>,
    scalar_units: Box<[ScalarUnitCounters]>,
    scalar_index: BTreeMap<(InstructionOffset, ScalarBlockPlanId), usize>,
}

#[derive(Clone, Copy, Debug)]
struct SuperinstructionCounters {
    offset: InstructionOffset,
    kind: SelectedPhysicalUnitKind,
    covered_operations: u16,
}

#[derive(Debug)]
struct ScalarUnitCounters {
    offset: InstructionOffset,
    plan: ScalarBlockPlanId,
    operation_sources: Box<[ScalarSourcePointId]>,
    subpoint_hits: Box<[AtomicU64]>,
    loop_counters: Option<ScalarLoopCounters>,
}

#[derive(Debug)]
struct ScalarLoopCounters {
    entries: AtomicU64,
    iterations: AtomicU64,
    exits: AtomicU64,
    charged_backedges: AtomicU64,
}

impl GenerationBytecodeProfile {
    pub(super) fn for_layout(layout: &ProfileLayout) -> Self {
        let functions = layout
            .functions()
            .iter()
            .map(|function| {
                let reserved_offsets = function
                    .selected_units
                    .iter()
                    .map(|unit| unit.offset)
                    .chain(function.scalar_units.iter().map(|unit| unit.offset))
                    .collect::<BTreeSet<_>>();
                let ordinary_offsets = (0..function.instruction_count)
                    .map(InstructionOffset)
                    .filter(|offset| !reserved_offsets.contains(offset))
                    .collect::<Box<[_]>>();
                let superinstructions = function
                    .selected_units
                    .iter()
                    .map(|unit| SuperinstructionCounters {
                        offset: unit.offset,
                        kind: unit.kind,
                        covered_operations: unit.covered_operations,
                    })
                    .collect::<Box<[_]>>();
                let scalar_units = function
                    .scalar_units
                    .iter()
                    .map(|unit| ScalarUnitCounters {
                        offset: unit.offset,
                        plan: unit.plan,
                        operation_sources: unit.operation_sources.clone(),
                        subpoint_hits: (0..unit.source_count).map(|_| AtomicU64::new(0)).collect(),
                        loop_counters: unit.has_range_loop.then(ScalarLoopCounters::new),
                    })
                    .collect::<Box<[_]>>();
                let scalar_index = scalar_units
                    .iter()
                    .enumerate()
                    .map(|(index, unit)| ((unit.offset, unit.plan), index))
                    .collect();
                FunctionCounters {
                    handle: function.handle,
                    debug_name: function.debug_name,
                    instruction_hits: (0..function.instruction_count)
                        .map(|_| AtomicU64::new(0))
                        .collect(),
                    ordinary_offsets,
                    superinstructions,
                    scalar_units,
                    scalar_index,
                }
            })
            .collect::<Box<[_]>>();
        let function_index = functions
            .iter()
            .enumerate()
            .map(|(index, function)| (function.debug_name, index))
            .collect();
        Self {
            functions,
            function_index,
        }
    }

    pub(super) fn snapshot(&self, generation: ExecutableGenerationId) -> BytecodeProfileSnapshot {
        BytecodeProfileSnapshot {
            generation,
            functions: self
                .functions
                .iter()
                .map(FunctionCounters::snapshot)
                .collect(),
        }
    }

    pub(super) fn reset(&self) {
        for counter in self
            .functions
            .iter()
            .flat_map(|function| function.instruction_hits.iter())
        {
            counter.store(0, Ordering::Relaxed);
        }
        for scalar in self
            .functions
            .iter()
            .flat_map(|function| function.scalar_units.iter())
        {
            for counter in &scalar.subpoint_hits {
                counter.store(0, Ordering::Relaxed);
            }
            if let Some(counters) = &scalar.loop_counters {
                counters.reset();
            }
        }
    }

    #[cfg(test)]
    pub(super) fn set_instruction_hit_count(
        &self,
        function: DebugNameId,
        offset: InstructionOffset,
        value: u64,
    ) -> bool {
        let Some(index) = self.function_index.get(&function).copied() else {
            return false;
        };
        let Some(count) = self
            .functions
            .get(index)
            .and_then(|function| function.instruction_hits.get(offset.0))
        else {
            return false;
        };
        count.store(value, Ordering::Relaxed);
        true
    }
}

impl VmBytecodeProfiler for GenerationBytecodeProfile {
    fn record_instruction(&self, function: DebugNameId, offset: InstructionOffset) {
        let Some(index) = self.function_index.get(&function).copied() else {
            return;
        };
        let Some(count) = self
            .functions
            .get(index)
            .and_then(|function| function.instruction_hits.get(offset.0))
        else {
            return;
        };
        increment(count);
    }

    fn record_scalar_subpoint(
        &self,
        function: DebugNameId,
        offset: InstructionOffset,
        plan: ScalarBlockPlanId,
        source: ScalarSourcePointId,
    ) {
        let Some(unit) = self.scalar_unit(function, offset, plan) else {
            return;
        };
        if let Some(count) = unit.subpoint_hits.get(source.index()) {
            increment(count);
        }
    }

    fn record_scalar_loop_event(
        &self,
        function: DebugNameId,
        offset: InstructionOffset,
        plan: ScalarBlockPlanId,
        event: ScalarLoopProfileEvent,
    ) {
        let Some(counters) = self
            .scalar_unit(function, offset, plan)
            .and_then(|unit| unit.loop_counters.as_ref())
        else {
            return;
        };
        increment(match event {
            ScalarLoopProfileEvent::Entry => &counters.entries,
            ScalarLoopProfileEvent::Iteration => &counters.iterations,
            ScalarLoopProfileEvent::Exit => &counters.exits,
            ScalarLoopProfileEvent::ChargedBackedge => &counters.charged_backedges,
        });
    }
}

impl GenerationBytecodeProfile {
    fn scalar_unit(
        &self,
        function: DebugNameId,
        offset: InstructionOffset,
        plan: ScalarBlockPlanId,
    ) -> Option<&ScalarUnitCounters> {
        let function = self.functions.get(*self.function_index.get(&function)?)?;
        function
            .scalar_units
            .get(*function.scalar_index.get(&(offset, plan))?)
    }
}

impl ScalarUnitCounters {
    fn snapshot(&self, instruction_hits: &[AtomicU64]) -> ScalarUnitBytecodeProfile {
        let subpoint_hits = self
            .subpoint_hits
            .iter()
            .map(|count| count.load(Ordering::Relaxed))
            .collect::<Vec<_>>();
        let compact_operation_hits = self.operation_sources.iter().fold(0_u64, |total, source| {
            total.saturating_add(subpoint_hits.get(source.index()).copied().unwrap_or(0))
        });
        ScalarUnitBytecodeProfile {
            offset: self.offset,
            plan: self.plan,
            entry_hits: instruction_hits
                .get(self.offset.0)
                .map(|count| count.load(Ordering::Relaxed))
                .unwrap_or(0),
            compact_operation_hits,
            subpoint_hits,
            loop_profile: self
                .loop_counters
                .as_ref()
                .map(ScalarLoopCounters::snapshot),
        }
    }
}

impl FunctionCounters {
    fn snapshot(&self) -> FunctionBytecodeProfile {
        let instruction_hits = self
            .instruction_hits
            .iter()
            .map(|count| count.load(Ordering::Relaxed))
            .collect::<Vec<_>>();
        let ordinary_instruction_hits =
            self.ordinary_offsets.iter().fold(0_u64, |total, offset| {
                total.saturating_add(instruction_hits.get(offset.0).copied().unwrap_or(0))
            });
        let superinstructions = self
            .superinstructions
            .iter()
            .map(|unit| {
                let hits = instruction_hits.get(unit.offset.0).copied().unwrap_or(0);
                SuperinstructionBytecodeProfile {
                    offset: unit.offset,
                    kind: unit.kind,
                    hits,
                    eliminated_dispatches: hits
                        .saturating_mul(u64::from(unit.covered_operations.saturating_sub(1))),
                }
            })
            .collect();
        FunctionBytecodeProfile {
            handle: self.handle,
            debug_name: self.debug_name,
            ordinary_instruction_hits,
            superinstructions,
            scalar_units: self
                .scalar_units
                .iter()
                .map(|unit| unit.snapshot(&self.instruction_hits))
                .collect(),
            instruction_hits,
        }
    }
}

impl ScalarLoopCounters {
    fn new() -> Self {
        Self {
            entries: AtomicU64::new(0),
            iterations: AtomicU64::new(0),
            exits: AtomicU64::new(0),
            charged_backedges: AtomicU64::new(0),
        }
    }

    fn snapshot(&self) -> ScalarLoopBytecodeProfile {
        ScalarLoopBytecodeProfile {
            entries: self.entries.load(Ordering::Relaxed),
            iterations: self.iterations.load(Ordering::Relaxed),
            exits: self.exits.load(Ordering::Relaxed),
            charged_backedges: self.charged_backedges.load(Ordering::Relaxed),
        }
    }

    fn reset(&self) {
        self.entries.store(0, Ordering::Relaxed);
        self.iterations.store(0, Ordering::Relaxed);
        self.exits.store(0, Ordering::Relaxed);
        self.charged_backedges.store(0, Ordering::Relaxed);
    }
}

fn increment(count: &AtomicU64) {
    let _ = count.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(1))
    });
}
