use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use vela_bytecode::{
    DebugNameId, ExecutableGenerationId, InstructionOffset, ProfileLayout, ScalarBlockPlanId,
    ScalarSourcePointId, ScriptFunctionHandle,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionBytecodeProfile {
    handle: ScriptFunctionHandle,
    debug_name: DebugNameId,
    instruction_hits: Vec<u64>,
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
    pub fn scalar_units(&self) -> &[ScalarUnitBytecodeProfile] {
        &self.scalar_units
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarUnitBytecodeProfile {
    offset: InstructionOffset,
    plan: ScalarBlockPlanId,
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
    scalar_units: Box<[ScalarUnitCounters]>,
    scalar_index: BTreeMap<(InstructionOffset, ScalarBlockPlanId), usize>,
}

#[derive(Debug)]
struct ScalarUnitCounters {
    offset: InstructionOffset,
    plan: ScalarBlockPlanId,
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
                let scalar_units = function
                    .scalar_units
                    .iter()
                    .map(|unit| ScalarUnitCounters {
                        offset: unit.offset,
                        plan: unit.plan,
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
                .map(|function| FunctionBytecodeProfile {
                    handle: function.handle,
                    debug_name: function.debug_name,
                    instruction_hits: function
                        .instruction_hits
                        .iter()
                        .map(|count| count.load(Ordering::Relaxed))
                        .collect(),
                    scalar_units: function
                        .scalar_units
                        .iter()
                        .map(ScalarUnitCounters::snapshot)
                        .collect(),
                })
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
    fn snapshot(&self) -> ScalarUnitBytecodeProfile {
        ScalarUnitBytecodeProfile {
            offset: self.offset,
            plan: self.plan,
            subpoint_hits: self
                .subpoint_hits
                .iter()
                .map(|count| count.load(Ordering::Relaxed))
                .collect(),
            loop_profile: self
                .loop_counters
                .as_ref()
                .map(ScalarLoopCounters::snapshot),
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
