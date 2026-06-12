use super::*;
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Default)]
struct RecordingBytecodeProfiler {
    hits: RefCell<BTreeMap<(vela_bytecode::DebugNameId, InstructionOffset), u64>>,
}

impl RecordingBytecodeProfiler {
    fn hit_count(&self, function: vela_bytecode::DebugNameId, offset: InstructionOffset) -> u64 {
        self.hits
            .borrow()
            .get(&(function, offset))
            .copied()
            .unwrap_or(0)
    }

    fn function_hit_count(&self, function: vela_bytecode::DebugNameId) -> u64 {
        self.hits
            .borrow()
            .iter()
            .filter_map(|(&(hit_function, _), count)| (hit_function == function).then_some(count))
            .sum()
    }
}

impl VmBytecodeProfiler for RecordingBytecodeProfiler {
    fn record_instruction(&self, function: vela_bytecode::DebugNameId, offset: InstructionOffset) {
        *self
            .hits
            .borrow_mut()
            .entry((function, offset))
            .or_default() += 1;
    }
}

#[test]
fn linked_bytecode_profiler_records_direct_closure_body_offsets() {
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let closure_name = program.intern_debug_name("main::<closure#0>");
    let closure_function = vela_bytecode::ScriptFunctionHandle::new(1);

    let mut main = vela_bytecode::LinkedCodeObject::new(main_name, 2);
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeClosure {
            dst: Register(0),
            function: closure_function,
            captures: Vec::new(),
        },
    ));
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallClosure {
            dst: Register(1),
            callee: Register(0),
            args: Vec::new(),
        },
    ));
    main.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(1) },
    ));

    let mut closure = vela_bytecode::LinkedCodeObject::new(closure_name, 1);
    let value = closure.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(42)));
    closure.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    closure.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(0) },
    ));

    let main_function = program.push_function(main);
    assert_eq!(program.push_function(closure), closure_function);
    program.set_entry_point(main_name, main_function);

    let profiler = RecordingBytecodeProfiler::default();
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::unbounded();
    let code = program.function(main_function).expect("main function");
    let result = Vm::new()
        .execute_linked_call(
            crate::linked_execution::LinkedExecutionCall {
                code,
                program: &program,
                captures: &[],
                args: &[],
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: None,
                bytecode_profiler: Some(&profiler),
            },
            None,
            Some(&mut heap_execution),
            Some(&mut budget),
        )
        .expect("linked closure should execute");

    assert_eq!(result, Value::Scalar(vela_common::ScalarValue::I64(42)));
    assert_eq!(profiler.hit_count(main_name, InstructionOffset(0)), 1);
    assert_eq!(profiler.hit_count(main_name, InstructionOffset(1)), 1);
    assert_eq!(profiler.hit_count(closure_name, InstructionOffset(0)), 1);
    assert_eq!(profiler.hit_count(closure_name, InstructionOffset(1)), 1);
}

#[test]
fn linked_bytecode_profiler_records_callback_body_offsets() {
    let program = compile_standard_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let mapped = [1, 2, 3].map(|value| value + 1);
    return mapped[0];
}
"#,
    )
    .expect("standard callback method source should compile");
    let linked = link_test_program(&program);
    let main_function = linked.entry_point_by_name("main").expect("main entry");
    let main_code = linked.function(main_function).expect("main function");
    let callback_function = linked
        .functions()
        .find_map(|(function, code)| (function != main_function).then_some(code.debug_name))
        .expect("callback function should be linked");
    let profiler = RecordingBytecodeProfiler::default();
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::unbounded();

    let result = Vm::new()
        .execute_linked_call(
            crate::linked_execution::LinkedExecutionCall {
                code: main_code,
                program: &linked,
                captures: &[],
                args: &[],
                check_param_guards: true,
                call_site: None,
                call_site_offset: None,
                inline_caches: None,
                bytecode_profiler: Some(&profiler),
            },
            None,
            Some(&mut heap_execution),
            Some(&mut budget),
        )
        .expect("linked callback method should execute");

    assert_eq!(result, Value::Scalar(vela_common::ScalarValue::I64(2)));
    assert!(
        profiler.function_hit_count(callback_function) > 0,
        "callback body should report linked bytecode hits"
    );
}
