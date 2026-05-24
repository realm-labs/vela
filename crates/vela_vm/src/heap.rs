//! Non-moving script heap and mark-sweep collection.

use std::collections::BTreeMap;
use std::fmt;
use std::mem;

use vela_host::HostRef;

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
pub enum HeapSlot {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Ref(GcRef),
    HostRef(HostRef),
}

#[derive(Clone, Debug, PartialEq)]
pub enum HeapValue {
    String(String),
    Array(Vec<HeapSlot>),
    Map(BTreeMap<String, HeapSlot>),
    Set(Vec<HeapSlot>),
    Record {
        type_name: String,
        fields: BTreeMap<String, HeapSlot>,
    },
    Enum {
        enum_name: String,
        variant: String,
        fields: BTreeMap<String, HeapSlot>,
    },
}

impl HeapValue {
    fn trace_refs(&self, refs: &mut Vec<GcRef>) {
        match self {
            Self::String(_) => {}
            Self::Array(values) | Self::Set(values) => {
                values.iter().for_each(|value| value.trace_refs(refs));
            }
            Self::Map(values) => {
                values.values().for_each(|value| value.trace_refs(refs));
            }
            Self::Record { fields, .. } | Self::Enum { fields, .. } => {
                fields.values().for_each(|value| value.trace_refs(refs));
            }
        }
    }

    fn shallow_size_bytes(&self) -> usize {
        match self {
            Self::String(value) => mem::size_of::<Self>() + value.len(),
            Self::Array(values) | Self::Set(values) => {
                mem::size_of::<Self>() + values.capacity() * mem::size_of::<HeapSlot>()
            }
            Self::Map(values) => {
                mem::size_of::<Self>()
                    + values
                        .keys()
                        .map(|key| key.len() + mem::size_of::<HeapSlot>())
                        .sum::<usize>()
            }
            Self::Record { type_name, fields } => {
                mem::size_of::<Self>()
                    + type_name.len()
                    + fields
                        .keys()
                        .map(|key| key.len() + mem::size_of::<HeapSlot>())
                        .sum::<usize>()
            }
            Self::Enum {
                enum_name,
                variant,
                fields,
            } => {
                mem::size_of::<Self>()
                    + enum_name.len()
                    + variant.len()
                    + fields
                        .keys()
                        .map(|key| key.len() + mem::size_of::<HeapSlot>())
                        .sum::<usize>()
            }
        }
    }
}

impl HeapSlot {
    fn trace_refs(&self, refs: &mut Vec<GcRef>) {
        if let Self::Ref(gc_ref) = self {
            refs.push(*gc_ref);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GcStats {
    pub marked: usize,
    pub swept: usize,
    pub bytes_freed: usize,
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

#[derive(Clone, Debug, Default)]
pub struct ScriptHeap {
    entries: Vec<HeapEntry>,
    free_list: Vec<usize>,
    allocated_bytes: usize,
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

    pub fn collect_full(&mut self, roots: &[GcRef]) -> GcStats {
        self.collect_full_with_budget(roots, None)
    }

    pub fn collect_full_with_budget(
        &mut self,
        roots: &[GcRef],
        mut budget: Option<&mut ExecutionBudget>,
    ) -> GcStats {
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

        GcStats {
            marked,
            swept,
            bytes_freed,
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
        let mut stack = roots.to_vec();

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

        marked
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

    fn host_ref() -> HostRef {
        HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3)
    }

    #[test]
    fn live_script_objects_survive_full_gc() {
        let mut heap = ScriptHeap::new();
        let child = heap.allocate(HeapValue::String("gold".into()));
        let root = heap.allocate(HeapValue::Array(vec![HeapSlot::Ref(child)]));

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
        let second = heap.allocate(HeapValue::Array(vec![HeapSlot::Ref(first)]));
        let HeapValue::Array(values) = heap.get_mut(first).expect("first object") else {
            panic!("expected array");
        };
        values.push(HeapSlot::Ref(second));

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
        let root = heap.allocate(HeapValue::Array(vec![HeapSlot::HostRef(host_ref())]));
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
        let mut budget = ExecutionBudget::new(u64::MAX, 8, usize::MAX, usize::MAX);

        let error = heap
            .allocate_with_budget(HeapValue::String("this is too large".into()), &mut budget)
            .expect_err("allocation should exceed memory budget");

        assert_eq!(heap.live_object_count(), 0);
        assert_eq!(budget.memory_bytes_allocated(), 0);
        assert_eq!(
            error.kind,
            crate::VmErrorKind::BudgetExceeded {
                budget: crate::ExecutionBudgetKind::MemoryBytes,
                limit: 8,
            }
        );
    }

    #[test]
    fn full_gc_releases_memory_budget_for_swept_objects() {
        let mut heap = ScriptHeap::new();
        let mut budget = ExecutionBudget::new(u64::MAX, 1024, usize::MAX, usize::MAX);
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
}
