//! Non-moving script heap and mark-sweep collection.

use std::collections::BTreeMap;
use std::fmt;
use std::mem;

use vela_def::{FieldId, TypeId, VariantId};
use vela_host::proxy::PathProxy;

use crate::iteration::IteratorState;
use crate::script_object::ScriptFields;
use crate::value::{ClosureValue, Value};
use crate::{ExecutionBudget, VmResult};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GcRef {
    index: u32,
    generation: u32,
}

impl GcRef {
    #[must_use]
    pub const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum HeapValue {
    String(String),
    Array(Vec<Value>),
    Map(BTreeMap<String, Value>),
    Set(Vec<Value>),
    Record {
        type_name: String,
        fields: ScriptFields<Value>,
    },
    Enum {
        enum_name: String,
        variant: String,
        identity: Option<EnumIdentity>,
        fields: ScriptFields<Value>,
    },
    Closure(ClosureValue),
    Iterator(IteratorState),
    PathProxy(PathProxy),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnumIdentity {
    pub type_id: TypeId,
    pub variant_id: VariantId,
    pub payload_field_id: Option<FieldId>,
}

impl EnumIdentity {
    #[must_use]
    pub const fn new(
        type_id: TypeId,
        variant_id: VariantId,
        payload_field_id: Option<FieldId>,
    ) -> Self {
        Self {
            type_id,
            variant_id,
            payload_field_id,
        }
    }
}

impl HeapValue {
    fn trace_refs(&self, refs: &mut Vec<GcRef>) {
        match self {
            Self::String(_) | Self::PathProxy(_) => {}
            Self::Array(values) | Self::Set(values) => {
                values.iter().for_each(|value| value.trace_refs(refs));
            }
            Self::Map(values) => {
                values.values().for_each(|value| value.trace_refs(refs));
            }
            Self::Record { fields, .. } | Self::Enum { fields, .. } => {
                fields.values().for_each(|value| value.trace_refs(refs));
            }
            Self::Closure(closure) => {
                closure
                    .captures
                    .iter()
                    .for_each(|value| value.trace_refs(refs));
            }
            Self::Iterator(iterator) => iterator.trace_heap_refs(refs),
        }
    }

    fn shallow_size_bytes(&self) -> usize {
        match self {
            Self::String(value) => mem::size_of::<Self>() + value.len(),
            Self::Array(values) | Self::Set(values) => {
                mem::size_of::<Self>() + values.capacity() * mem::size_of::<Value>()
            }
            Self::Map(values) => {
                mem::size_of::<Self>()
                    + values
                        .keys()
                        .map(|key| key.len() + mem::size_of::<Value>())
                        .sum::<usize>()
            }
            Self::Record { type_name, fields } => {
                mem::size_of::<Self>()
                    + type_name.len()
                    + fields
                        .iter()
                        .map(|(key, _)| key.len() + mem::size_of::<Value>())
                        .sum::<usize>()
            }
            Self::Enum {
                enum_name,
                variant,
                fields,
                ..
            } => {
                mem::size_of::<Self>()
                    + enum_name.len()
                    + variant.len()
                    + fields
                        .iter()
                        .map(|(key, _)| key.len() + mem::size_of::<Value>())
                        .sum::<usize>()
            }
            Self::Closure(closure) => {
                mem::size_of::<Self>() + closure.captures.capacity() * mem::size_of::<Value>()
            }
            Self::Iterator(iterator) => {
                mem::size_of::<Self>() + mem::size_of_val(iterator.values())
            }
            Self::PathProxy(_) => mem::size_of::<Self>(),
        }
    }
}

impl Value {
    fn trace_refs(&self, refs: &mut Vec<GcRef>) {
        self.trace_heap_refs(refs);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GcStats {
    pub marked: usize,
    pub swept: usize,
    pub bytes_freed: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GcConfig {
    pub max_pause_micros: u64,
    pub heap_growth_factor: f64,
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            max_pause_micros: 500,
            heap_growth_factor: 1.5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GcBudget {
    pub max_sweep_slots: usize,
    pub max_pause_micros: u64,
}

impl GcBudget {
    #[must_use]
    pub const fn sweep_slots(max_sweep_slots: usize) -> Self {
        Self {
            max_sweep_slots,
            max_pause_micros: 0,
        }
    }

    #[must_use]
    pub const fn micros(max_pause_micros: u64) -> Self {
        Self {
            max_sweep_slots: usize::MAX,
            max_pause_micros,
        }
    }

    #[must_use]
    pub const fn unlimited() -> Self {
        Self {
            max_sweep_slots: usize::MAX,
            max_pause_micros: u64::MAX,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GcStepStats {
    pub marked: usize,
    pub sweep_slots_visited: usize,
    pub swept: usize,
    pub bytes_freed: usize,
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeapError {
    pub kind: HeapErrorKind,
}

impl HeapError {
    fn new(kind: HeapErrorKind) -> Self {
        Self { kind }
    }
}

impl fmt::Display for HeapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", self.kind)
    }
}

impl std::error::Error for HeapError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeapErrorKind {
    InvalidRef { reference: GcRef },
}

pub type HeapResult<T> = Result<T, HeapError>;

#[derive(Clone, Debug)]
struct HeapObject {
    value: HeapValue,
    marked: bool,
    size_bytes: usize,
}

#[derive(Clone, Debug, Default)]
struct HeapEntry {
    generation: u32,
    object: Option<HeapObject>,
}

#[derive(Clone, Debug)]
struct IncrementalGc {
    sweep_index: usize,
}

#[derive(Clone, Debug)]
pub struct ScriptHeap {
    entries: Vec<HeapEntry>,
    free_list: Vec<usize>,
    mark_stack: Vec<GcRef>,
    allocated_bytes: usize,
    gc_config: GcConfig,
    next_gc_at_bytes: usize,
    incremental_gc: Option<IncrementalGc>,
}

impl Default for ScriptHeap {
    fn default() -> Self {
        let gc_config = GcConfig::default();
        Self {
            entries: Vec::new(),
            free_list: Vec::new(),
            mark_stack: Vec::new(),
            allocated_bytes: 0,
            gc_config,
            next_gc_at_bytes: 1,
            incremental_gc: None,
        }
    }
}

impl ScriptHeap {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate(&mut self, value: HeapValue) -> GcRef {
        let size_bytes = value.shallow_size_bytes();
        self.allocate_object(value, size_bytes)
    }

    pub fn allocate_with_budget(
        &mut self,
        value: HeapValue,
        budget: &mut ExecutionBudget,
    ) -> VmResult<GcRef> {
        let size_bytes = value.shallow_size_bytes();
        budget.charge_memory(size_bytes)?;
        Ok(self.allocate_object(value, size_bytes))
    }

    #[must_use]
    pub fn get(&self, reference: GcRef) -> Option<&HeapValue> {
        self.entry(reference)
            .and_then(|entry| entry.object.as_ref().map(|object| &object.value))
    }

    pub fn get_mut(&mut self, reference: GcRef) -> HeapResult<&mut HeapValue> {
        self.entry_mut(reference)
            .and_then(|entry| entry.object.as_mut())
            .map(|object| &mut object.value)
            .ok_or_else(|| HeapError::new(HeapErrorKind::InvalidRef { reference }))
    }

    #[must_use]
    pub fn contains(&self, reference: GcRef) -> bool {
        self.get(reference).is_some()
    }

    #[must_use]
    pub fn live_object_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.object.is_some())
            .count()
    }

    #[must_use]
    pub fn allocated_bytes(&self) -> usize {
        self.allocated_bytes
    }

    #[must_use]
    pub fn gc_config(&self) -> GcConfig {
        self.gc_config
    }

    pub fn set_gc_config(&mut self, config: GcConfig) {
        self.gc_config = config;
        self.update_next_gc_threshold();
    }

    #[must_use]
    pub fn next_gc_at_bytes(&self) -> usize {
        self.next_gc_at_bytes
    }

    #[must_use]
    pub fn should_collect(&self) -> bool {
        self.allocated_bytes >= self.next_gc_at_bytes
    }

    pub fn collect_full(&mut self, roots: &[GcRef]) -> GcStats {
        self.collect_full_with_budget(roots, None)
    }

    pub fn collect_full_with_budget(
        &mut self,
        roots: &[GcRef],
        mut budget: Option<&mut ExecutionBudget>,
    ) -> GcStats {
        self.incremental_gc = None;
        self.clear_marks();
        let marked = self.mark_from_roots(roots);
        let mut swept = 0;
        let mut bytes_freed = 0;

        for (index, entry) in self.entries.iter_mut().enumerate() {
            let Some(object) = entry.object.as_mut() else {
                continue;
            };
            if object.marked {
                object.marked = false;
                continue;
            }

            let object = entry.object.take().expect("checked object exists");
            bytes_freed += object.size_bytes;
            swept += 1;
            self.free_list.push(index);
        }

        self.allocated_bytes = self.allocated_bytes.saturating_sub(bytes_freed);
        if let Some(budget) = &mut budget {
            budget.release_memory(bytes_freed);
        }
        self.update_next_gc_threshold();

        GcStats {
            marked,
            swept,
            bytes_freed,
        }
    }

    pub fn step_gc(&mut self, roots: &[GcRef], budget: GcBudget) -> GcStepStats {
        self.step_gc_with_budget(roots, budget, None)
    }

    pub fn step_gc_with_budget(
        &mut self,
        roots: &[GcRef],
        budget: GcBudget,
        mut execution_budget: Option<&mut ExecutionBudget>,
    ) -> GcStepStats {
        let marked = if self.incremental_gc.is_some() {
            0
        } else {
            self.clear_marks();
            let marked = self.mark_from_roots(roots);
            self.incremental_gc = Some(IncrementalGc { sweep_index: 0 });
            marked
        };

        let mut sweep_slots_visited = 0;
        let mut swept = 0;
        let mut bytes_freed = 0;
        let mut complete = false;

        while sweep_slots_visited < budget.max_sweep_slots {
            let Some(state) = self.incremental_gc.as_mut() else {
                complete = true;
                break;
            };
            if state.sweep_index >= self.entries.len() {
                self.incremental_gc = None;
                complete = true;
                break;
            }

            let index = state.sweep_index;
            state.sweep_index += 1;
            sweep_slots_visited += 1;

            let Some(object) = self.entries[index].object.as_mut() else {
                continue;
            };
            if object.marked {
                object.marked = false;
                continue;
            }

            let object = self.entries[index]
                .object
                .take()
                .expect("checked object exists");
            bytes_freed += object.size_bytes;
            swept += 1;
            self.free_list.push(index);
        }

        if !complete
            && self
                .incremental_gc
                .as_ref()
                .is_some_and(|state| state.sweep_index >= self.entries.len())
        {
            self.incremental_gc = None;
            complete = true;
        }

        self.allocated_bytes = self.allocated_bytes.saturating_sub(bytes_freed);
        if let Some(execution_budget) = &mut execution_budget {
            execution_budget.release_memory(bytes_freed);
        }
        if complete {
            self.update_next_gc_threshold();
        }

        GcStepStats {
            marked,
            sweep_slots_visited,
            swept,
            bytes_freed,
            complete,
        }
    }

    fn allocate_object(&mut self, value: HeapValue, size_bytes: usize) -> GcRef {
        let object = HeapObject {
            value,
            marked: false,
            size_bytes,
        };
        self.allocated_bytes = self.allocated_bytes.saturating_add(size_bytes);

        if let Some(index) = self.free_list.pop() {
            let entry = &mut self.entries[index];
            entry.generation = entry.generation.saturating_add(1).max(1);
            entry.object = Some(object);
            return GcRef::new(u32::try_from(index).unwrap_or(u32::MAX), entry.generation);
        }

        let index = self.entries.len();
        self.entries.push(HeapEntry {
            generation: 1,
            object: Some(object),
        });
        GcRef::new(u32::try_from(index).unwrap_or(u32::MAX), 1)
    }

    fn mark_from_roots(&mut self, roots: &[GcRef]) -> usize {
        let mut marked = 0;
        let mut stack = std::mem::take(&mut self.mark_stack);
        stack.clear();
        stack.extend_from_slice(roots);

        while let Some(reference) = stack.pop() {
            let Some(object) = self
                .entry_mut(reference)
                .and_then(|entry| entry.object.as_mut())
            else {
                continue;
            };
            if object.marked {
                continue;
            }

            object.marked = true;
            marked += 1;
            object.value.trace_refs(&mut stack);
        }

        self.mark_stack = stack;
        marked
    }

    fn clear_marks(&mut self) {
        for object in self
            .entries
            .iter_mut()
            .filter_map(|entry| entry.object.as_mut())
        {
            object.marked = false;
        }
    }

    fn update_next_gc_threshold(&mut self) {
        let factor = self.gc_config.heap_growth_factor.max(1.0);
        let grown = (self.allocated_bytes as f64 * factor).ceil() as usize;
        self.next_gc_at_bytes = grown.max(self.allocated_bytes.saturating_add(1));
    }

    fn entry(&self, reference: GcRef) -> Option<&HeapEntry> {
        let index = usize::try_from(reference.index).ok()?;
        let entry = self.entries.get(index)?;
        (entry.generation == reference.generation).then_some(entry)
    }

    fn entry_mut(&mut self, reference: GcRef) -> Option<&mut HeapEntry> {
        let index = usize::try_from(reference.index).ok()?;
        let entry = self.entries.get_mut(index)?;
        (entry.generation == reference.generation).then_some(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_common::{HostObjectId, HostTypeId};
    use vela_def::FieldId;
    use vela_host::path::{HostPath, HostRef};

    fn host_ref() -> HostRef {
        HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3)
    }

    #[test]
    fn live_script_objects_survive_full_gc() {
        let mut heap = ScriptHeap::new();
        let child = heap.allocate(HeapValue::String("gold".into()));
        let root = heap.allocate(HeapValue::Array(vec![Value::HeapRef(child)]));

        let stats = heap.collect_full(&[root]);

        assert_eq!(stats.marked, 2);
        assert_eq!(stats.swept, 0);
        assert!(heap.contains(root));
        assert!(heap.contains(child));
        assert_eq!(heap.live_object_count(), 2);
    }

    #[test]
    fn cyclic_script_objects_are_reclaimed_without_roots() {
        let mut heap = ScriptHeap::new();
        let first = heap.allocate(HeapValue::Array(Vec::new()));
        let second = heap.allocate(HeapValue::Array(vec![Value::HeapRef(first)]));
        let HeapValue::Array(values) = heap.get_mut(first).expect("first object") else {
            panic!("expected array");
        };
        values.push(Value::HeapRef(second));

        let stats = heap.collect_full(&[]);

        assert_eq!(stats.marked, 0);
        assert_eq!(stats.swept, 2);
        assert!(!heap.contains(first));
        assert!(!heap.contains(second));
        assert_eq!(heap.live_object_count(), 0);
    }

    #[test]
    fn host_refs_are_external_and_do_not_trace_rust_owned_state() {
        let mut heap = ScriptHeap::new();
        let root = heap.allocate(HeapValue::Array(vec![Value::HostRef(host_ref())]));
        let unreachable = heap.allocate(HeapValue::String("unused".into()));

        let stats = heap.collect_full(&[root]);

        assert_eq!(stats.marked, 1);
        assert_eq!(stats.swept, 1);
        assert!(heap.contains(root));
        assert!(!heap.contains(unreachable));
    }

    #[test]
    fn path_proxies_are_external_and_do_not_trace_rust_owned_state() {
        let mut heap = ScriptHeap::new();
        let proxy =
            PathProxy::from_diagnostic_path(HostPath::new(host_ref()).field(FieldId::new(2)));
        let root = heap.allocate(HeapValue::PathProxy(proxy));
        let unreachable = heap.allocate(HeapValue::String("unused".into()));

        let stats = heap.collect_full(&[root]);

        assert_eq!(stats.marked, 1);
        assert_eq!(stats.swept, 1);
        assert!(heap.contains(root));
        assert!(!heap.contains(unreachable));
    }

    #[test]
    fn stale_refs_do_not_access_reused_slots() {
        let mut heap = ScriptHeap::new();
        let old_ref = heap.allocate(HeapValue::String("old".into()));
        heap.collect_full(&[]);
        let new_ref = heap.allocate(HeapValue::String("new".into()));

        assert_ne!(old_ref, new_ref);
        assert_eq!(old_ref.index(), new_ref.index());
        assert!(heap.get(old_ref).is_none());
        assert_eq!(heap.get(new_ref), Some(&HeapValue::String("new".into())));
    }

    #[test]
    fn memory_budget_rejects_allocations_before_heap_mutation() {
        let mut heap = ScriptHeap::new();
        let mut budget = ExecutionBudget::new(u64::MAX, 8, usize::MAX);

        let error = heap
            .allocate_with_budget(HeapValue::String("this is too large".into()), &mut budget)
            .expect_err("allocation should exceed memory budget");

        assert_eq!(heap.live_object_count(), 0);
        assert_eq!(budget.memory_bytes_allocated(), 0);
        assert_eq!(
            error.kind(),
            crate::VmErrorKind::BudgetExceeded {
                budget: crate::budget::ExecutionBudgetKind::MemoryBytes,
                limit: 8,
            }
        );
    }

    #[test]
    fn full_gc_releases_memory_budget_for_swept_objects() {
        let mut heap = ScriptHeap::new();
        let mut budget = ExecutionBudget::new(u64::MAX, 1024, usize::MAX);
        let root = heap
            .allocate_with_budget(HeapValue::String("live".into()), &mut budget)
            .expect("root allocation");
        let _garbage = heap
            .allocate_with_budget(HeapValue::String("garbage".into()), &mut budget)
            .expect("garbage allocation");
        let before = budget.memory_bytes_allocated();

        let stats = heap.collect_full_with_budget(&[root], Some(&mut budget));

        assert_eq!(stats.swept, 1);
        assert!(stats.bytes_freed > 0);
        assert!(budget.memory_bytes_allocated() < before);
        assert_eq!(budget.memory_bytes_allocated(), heap.allocated_bytes());
    }

    #[test]
    fn step_gc_sweeps_with_slot_budget_across_calls() {
        let mut heap = ScriptHeap::new();
        let first = heap.allocate(HeapValue::String("first".into()));
        let second = heap.allocate(HeapValue::String("second".into()));
        let third = heap.allocate(HeapValue::String("third".into()));

        let first_step = heap.step_gc(&[], GcBudget::sweep_slots(1));
        assert_eq!(
            first_step,
            GcStepStats {
                marked: 0,
                sweep_slots_visited: 1,
                swept: 1,
                bytes_freed: first_step.bytes_freed,
                complete: false,
            }
        );
        assert!(!heap.contains(first));
        assert!(heap.contains(second));
        assert!(heap.contains(third));

        let second_step = heap.step_gc(&[], GcBudget::sweep_slots(1));
        assert_eq!(second_step.marked, 0);
        assert_eq!(second_step.sweep_slots_visited, 1);
        assert_eq!(second_step.swept, 1);
        assert!(!second_step.complete);
        assert!(!heap.contains(second));
        assert!(heap.contains(third));

        let final_step = heap.step_gc(&[], GcBudget::sweep_slots(1));
        assert_eq!(final_step.marked, 0);
        assert_eq!(final_step.sweep_slots_visited, 1);
        assert_eq!(final_step.swept, 1);
        assert!(final_step.complete);
        assert!(!heap.contains(third));
        assert_eq!(heap.live_object_count(), 0);
    }

    #[test]
    fn step_gc_preserves_roots_while_sweeping_incrementally() {
        let mut heap = ScriptHeap::new();
        let child = heap.allocate(HeapValue::String("child".into()));
        let root = heap.allocate(HeapValue::Array(vec![Value::HeapRef(child)]));
        let garbage = heap.allocate(HeapValue::String("garbage".into()));

        let first_step = heap.step_gc(&[root], GcBudget::sweep_slots(1));
        assert_eq!(first_step.marked, 2);
        assert_eq!(first_step.sweep_slots_visited, 1);
        assert_eq!(first_step.swept, 0);
        assert!(!first_step.complete);
        assert!(heap.contains(child));
        assert!(heap.contains(root));
        assert!(heap.contains(garbage));

        let second_step = heap.step_gc(&[root], GcBudget::unlimited());
        assert_eq!(second_step.marked, 0);
        assert_eq!(second_step.swept, 1);
        assert!(second_step.complete);
        assert!(heap.contains(child));
        assert!(heap.contains(root));
        assert!(!heap.contains(garbage));
    }

    #[test]
    fn step_gc_releases_execution_memory_budget_for_swept_objects() {
        let mut heap = ScriptHeap::new();
        let mut budget = ExecutionBudget::new(u64::MAX, 1024, usize::MAX);
        let root = heap
            .allocate_with_budget(HeapValue::String("live".into()), &mut budget)
            .expect("root allocation");
        let garbage = heap
            .allocate_with_budget(HeapValue::String("garbage".into()), &mut budget)
            .expect("garbage allocation");
        let before = budget.memory_bytes_allocated();

        let stats = heap.step_gc_with_budget(&[root], GcBudget::unlimited(), Some(&mut budget));

        assert!(stats.complete);
        assert_eq!(stats.swept, 1);
        assert!(heap.contains(root));
        assert!(!heap.contains(garbage));
        assert!(budget.memory_bytes_allocated() < before);
        assert_eq!(budget.memory_bytes_allocated(), heap.allocated_bytes());
    }

    #[test]
    fn full_gc_aborts_in_progress_step_and_restarts_from_current_roots() {
        let mut heap = ScriptHeap::new();
        let first = heap.allocate(HeapValue::String("first".into()));
        let second = heap.allocate(HeapValue::String("second".into()));
        let third = heap.allocate(HeapValue::String("third".into()));

        let partial = heap.step_gc(&[second], GcBudget::sweep_slots(1));
        assert!(!partial.complete);
        assert!(!heap.contains(first));
        assert!(heap.contains(second));
        assert!(heap.contains(third));

        let full = heap.collect_full(&[third]);

        assert_eq!(full.marked, 1);
        assert_eq!(full.swept, 1);
        assert!(!heap.contains(second));
        assert!(heap.contains(third));
    }

    #[test]
    fn gc_config_tracks_next_collection_threshold() {
        let mut heap = ScriptHeap::new();
        heap.set_gc_config(GcConfig {
            max_pause_micros: 200,
            heap_growth_factor: 1.0,
        });
        let live = heap.allocate(HeapValue::String("live".into()));

        let stats = heap.collect_full(&[live]);

        assert_eq!(stats.swept, 0);
        assert_eq!(heap.gc_config().max_pause_micros, 200);
        assert_eq!(heap.next_gc_at_bytes(), heap.allocated_bytes() + 1);
        assert!(!heap.should_collect());

        let _extra = heap.allocate(HeapValue::String("extra".into()));

        assert!(heap.should_collect());
    }
}
