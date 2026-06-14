use super::*;
use crate::value::Value as RuntimeValue;
use std::cell::{Cell, RefCell};

pub(super) fn run_linked_method_cache_program(
    program: &vela_bytecode::LinkedProgram,
    caches: &RecordingMethodCaches,
) -> VmResult<RuntimeValue> {
    run_linked_method_cache_runtime_value(program, caches)
}

pub(super) fn run_linked_method_cache_owned_program(
    program: &vela_bytecode::LinkedProgram,
    caches: &RecordingMethodCaches,
) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_method_cache_owned_program_with_budget(program, caches, &mut budget)
}

pub(super) fn run_linked_method_cache_owned_program_with_budget(
    program: &vela_bytecode::LinkedProgram,
    caches: &RecordingMethodCaches,
    budget: &mut ExecutionBudget,
) -> VmResult<OwnedValue> {
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let result =
        run_linked_method_cache_with_heap_and_budget(program, caches, &mut heap_execution, budget)?;
    crate::heap_values::value_to_owned(&result, Some(&heap_execution))
}

pub(super) fn run_linked_method_cache_program_with_standard_natives(
    program: &vela_bytecode::LinkedProgram,
    caches: &RecordingMethodCaches,
) -> VmResult<RuntimeValue> {
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let code = main_code(program);
    let mut budget = ExecutionBudget::unbounded();
    Vm::new().with_standard_natives().execute_linked_call(
        crate::linked_execution::LinkedExecutionCall {
            code,
            program,
            captures: &[],
            args: &[],
            check_param_guards: true,
            call_site: None,
            call_site_offset: None,
            inline_caches: Some(caches),
            bytecode_profiler: None,
        },
        None,
        Some(&mut heap_execution),
        Some(&mut budget),
    )
}

fn run_linked_method_cache_runtime_value(
    program: &vela_bytecode::LinkedProgram,
    caches: &RecordingMethodCaches,
) -> VmResult<RuntimeValue> {
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    run_linked_method_cache_with_heap(program, caches, &mut heap_execution)
}

fn run_linked_method_cache_with_heap(
    program: &vela_bytecode::LinkedProgram,
    caches: &RecordingMethodCaches,
    heap_execution: &mut HeapExecution<'_>,
) -> VmResult<RuntimeValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_method_cache_with_heap_and_budget(program, caches, heap_execution, &mut budget)
}

fn run_linked_method_cache_with_heap_and_budget(
    program: &vela_bytecode::LinkedProgram,
    caches: &RecordingMethodCaches,
    heap_execution: &mut HeapExecution<'_>,
    budget: &mut ExecutionBudget,
) -> VmResult<RuntimeValue> {
    let code = main_code(program);
    Vm::new().execute_linked_call(
        crate::linked_execution::LinkedExecutionCall {
            code,
            program,
            captures: &[],
            args: &[],
            check_param_guards: true,
            call_site: None,
            call_site_offset: None,
            inline_caches: Some(caches),
            bytecode_profiler: None,
        },
        None,
        Some(heap_execution),
        Some(budget),
    )
}

fn main_code(program: &vela_bytecode::LinkedProgram) -> &vela_bytecode::LinkedCodeObject {
    program
        .functions()
        .find(|(_, code)| program.debug_name(code.debug_name) == "main")
        .map(|(_, code)| code)
        .expect("linked method cache fixture should have main")
}

pub(super) struct RecordingMethodCaches {
    entries: RefCell<Vec<Option<MethodInlineCacheEntry>>>,
    site_set_counts: RefCell<Vec<usize>>,
    dynamic_entries: RefCell<Vec<Option<DynamicMethodInlineCacheEntry>>>,
    dynamic_site_get_counts: RefCell<Vec<usize>>,
    dynamic_site_set_counts: RefCell<Vec<usize>>,
    host_entries: RefCell<Vec<Option<HostInlineCacheEntry>>>,
    host_site_set_counts: RefCell<Vec<usize>>,
    set_count: Cell<usize>,
    dynamic_set_count: Cell<usize>,
    host_set_count: Cell<usize>,
}

impl RecordingMethodCaches {
    pub(super) fn new(len: usize) -> Self {
        Self {
            entries: RefCell::new(vec![None; len]),
            site_set_counts: RefCell::new(vec![0; len]),
            dynamic_entries: RefCell::new(vec![None; len]),
            dynamic_site_get_counts: RefCell::new(vec![0; len]),
            dynamic_site_set_counts: RefCell::new(vec![0; len]),
            host_entries: RefCell::new(vec![None; len]),
            host_site_set_counts: RefCell::new(vec![0; len]),
            set_count: Cell::new(0),
            dynamic_set_count: Cell::new(0),
            host_set_count: Cell::new(0),
        }
    }

    pub(super) fn entry(&self, site: CacheSiteId) -> Option<MethodInlineCacheEntry> {
        self.entries.borrow().get(site.index()).copied().flatten()
    }

    pub(super) fn prime(&self, site: CacheSiteId, entry: MethodInlineCacheEntry) {
        self.entries.borrow_mut()[site.index()] = Some(entry);
    }

    pub(super) fn set_count(&self) -> usize {
        self.set_count.get()
    }

    pub(super) fn set_count_for(&self, site: CacheSiteId) -> usize {
        self.site_set_counts.borrow()[site.index()]
    }

    pub(super) fn dynamic_entry(&self, site: CacheSiteId) -> Option<DynamicMethodInlineCacheEntry> {
        self.dynamic_entries
            .borrow()
            .get(site.index())
            .cloned()
            .flatten()
    }

    pub(super) fn dynamic_set_count(&self) -> usize {
        self.dynamic_set_count.get()
    }

    pub(super) fn dynamic_get_count_for(&self, site: CacheSiteId) -> usize {
        self.dynamic_site_get_counts.borrow()[site.index()]
    }

    pub(super) fn dynamic_set_count_for(&self, site: CacheSiteId) -> usize {
        self.dynamic_site_set_counts.borrow()[site.index()]
    }

    pub(super) fn host_entry(&self, site: CacheSiteId) -> Option<HostInlineCacheEntry> {
        self.host_entries
            .borrow()
            .get(site.index())
            .copied()
            .flatten()
    }

    pub(super) fn host_set_count_for(&self, site: CacheSiteId) -> usize {
        self.host_site_set_counts.borrow()[site.index()]
    }
}

pub(super) fn owned_option_some(value: OwnedValue) -> OwnedValue {
    OwnedValue::Enum {
        enum_name: "Option".to_owned(),
        variant: "Some".to_owned(),
        fields: ScriptFields::single("Option::Some", "0", value),
    }
}

impl VmInlineCaches for RecordingMethodCaches {
    fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    fn method_dispatch(&self, site: CacheSiteId) -> Option<MethodInlineCacheEntry> {
        self.entry(site)
    }

    fn set_method_dispatch(&self, site: CacheSiteId, entry: MethodInlineCacheEntry) {
        self.entries.borrow_mut()[site.index()] = Some(entry);
        self.site_set_counts.borrow_mut()[site.index()] += 1;
        self.set_count.set(self.set_count.get() + 1);
    }

    fn dynamic_method_dispatch(&self, site: CacheSiteId) -> Option<DynamicMethodInlineCacheEntry> {
        self.dynamic_site_get_counts.borrow_mut()[site.index()] += 1;
        self.dynamic_entry(site)
    }

    fn set_dynamic_method_dispatch(&self, site: CacheSiteId, entry: DynamicMethodInlineCacheEntry) {
        self.dynamic_entries.borrow_mut()[site.index()] = Some(entry);
        self.dynamic_site_set_counts.borrow_mut()[site.index()] += 1;
        self.dynamic_set_count.set(self.dynamic_set_count.get() + 1);
    }

    fn host_access(&self, site: CacheSiteId) -> Option<HostInlineCacheEntry> {
        self.host_entry(site)
    }

    fn set_host_access(&self, site: CacheSiteId, entry: HostInlineCacheEntry) {
        self.host_entries.borrow_mut()[site.index()] = Some(entry);
        self.host_site_set_counts.borrow_mut()[site.index()] += 1;
        self.host_set_count.set(self.host_set_count.get() + 1);
    }
}
