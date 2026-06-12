use super::linked_standard_method_cache_support::*;
use super::*;
use crate::value::Value as RuntimeValue;
use vela_stdlib_runtime::{StdFunctionImplementation, stdlib_function_runtime_bindings};

type LinkedSetCacheFixture = (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    MethodId,
);

#[test]
fn linked_standard_value_method_caches_set_values_target() {
    assert_set_owned_cache(
        linked_set_values_cache_program(),
        StandardMethodInlineCacheTarget::Values,
        OwnedValue::Array(vec![OwnedValue::i64(2), OwnedValue::i64(4)]),
    );
}

#[test]
fn linked_standard_value_method_caches_set_is_empty_target() {
    assert_set_runtime_cache(
        linked_set_no_arg_cache_program("is_empty", &[2, 4]),
        StandardMethodInlineCacheTarget::IsEmpty,
        RuntimeValue::Bool(false),
    );
}

#[test]
fn linked_standard_value_method_caches_set_add_target() {
    assert_set_runtime_cache(
        linked_set_mutator_cache_program("add", &[2], &[4]),
        StandardMethodInlineCacheTarget::Add,
        RuntimeValue::Bool(true),
    );
}

#[test]
fn linked_standard_value_method_caches_set_remove_target() {
    assert_set_runtime_cache(
        linked_set_mutator_cache_program("remove", &[2, 4], &[4]),
        StandardMethodInlineCacheTarget::Remove,
        RuntimeValue::Bool(true),
    );
}

#[test]
fn linked_standard_value_method_caches_set_clear_target() {
    assert_set_owned_cache(
        linked_set_no_arg_cache_program("clear", &[2, 4]),
        StandardMethodInlineCacheTarget::Clear,
        OwnedValue::Null,
    );
}

#[test]
fn linked_standard_value_method_caches_set_extend_target() {
    assert_set_owned_cache(
        linked_set_combination_cache_program("extend", &[2], &[4, 6]),
        StandardMethodInlineCacheTarget::Extend,
        OwnedValue::Null,
    );
}

#[test]
fn linked_standard_value_method_caches_set_self_extend_target() {
    let (program, site, dispatch, method_id) = linked_set_self_extend_cache_program();
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(OwnedValue::set([OwnedValue::i64(2), OwnedValue::i64(4)]));

    assert_eq!(
        run_linked_set_cache_owned_program(&program, &caches),
        expected
    );
    assert_set_cache_entry(
        &caches,
        site,
        dispatch,
        method_id,
        StandardMethodInlineCacheTarget::Extend,
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_set_empty_extend_target() {
    let (program, site, dispatch, method_id) =
        linked_set_extend_return_receiver_cache_program(&[2, 4], &[]);
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(OwnedValue::set([OwnedValue::i64(2), OwnedValue::i64(4)]));

    assert_eq!(
        run_linked_set_cache_owned_program(&program, &caches),
        expected
    );
    assert_set_cache_entry(
        &caches,
        site,
        dispatch,
        method_id,
        StandardMethodInlineCacheTarget::Extend,
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_set_pair_extend_target() {
    let (program, site, dispatch, method_id) =
        linked_set_extend_return_receiver_cache_program(&[2], &[4, 6]);
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(OwnedValue::set([
        OwnedValue::i64(2),
        OwnedValue::i64(4),
        OwnedValue::i64(6),
    ]));

    assert_eq!(
        run_linked_set_cache_owned_program(&program, &caches),
        expected
    );
    assert_set_cache_entry(
        &caches,
        site,
        dispatch,
        method_id,
        StandardMethodInlineCacheTarget::Extend,
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_set_union_target() {
    assert_set_owned_cache(
        linked_set_combination_cache_program("union", &[2, 4], &[4, 6]),
        StandardMethodInlineCacheTarget::Union,
        OwnedValue::set([OwnedValue::i64(2), OwnedValue::i64(4), OwnedValue::i64(6)]),
    );
}

#[test]
fn linked_standard_value_method_caches_set_intersection_target() {
    assert_set_owned_cache(
        linked_set_combination_cache_program("intersection", &[2, 4], &[4, 6]),
        StandardMethodInlineCacheTarget::Intersection,
        OwnedValue::set([OwnedValue::i64(4)]),
    );
}

#[test]
fn linked_standard_value_method_caches_set_difference_target() {
    assert_set_owned_cache(
        linked_set_combination_cache_program("difference", &[2, 4], &[4, 6]),
        StandardMethodInlineCacheTarget::Difference,
        OwnedValue::set([OwnedValue::i64(2)]),
    );
}

#[test]
fn linked_standard_value_method_caches_set_symmetric_difference_target() {
    assert_set_owned_cache(
        linked_set_combination_cache_program("symmetric_difference", &[2, 4], &[4, 6]),
        StandardMethodInlineCacheTarget::SymmetricDifference,
        OwnedValue::set([OwnedValue::i64(2), OwnedValue::i64(6)]),
    );
}

fn assert_set_owned_cache(
    fixture: LinkedSetCacheFixture,
    target: StandardMethodInlineCacheTarget,
    expected: OwnedValue,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(expected);

    assert_eq!(
        run_linked_set_cache_owned_program(&program, &caches),
        expected
    );
    assert_set_cache_entry(&caches, site, dispatch, method_id, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_set_cache_owned_program(&program, &caches),
        expected
    );
    assert_eq!(caches.set_count(), 2);
}

fn assert_set_runtime_cache(
    fixture: LinkedSetCacheFixture,
    target: StandardMethodInlineCacheTarget,
    expected: RuntimeValue,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(expected);

    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    assert_eq!(
        run_linked_set_cache_program(&program, &caches, &mut heap_execution),
        expected
    );
    assert_set_cache_entry(&caches, site, dispatch, method_id, target);
    assert_eq!(caches.set_count(), 2);

    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    assert_eq!(
        run_linked_set_cache_program(&program, &caches, &mut heap_execution),
        expected
    );
    assert_eq!(caches.set_count(), 2);
}

fn assert_set_cache_entry(
    caches: &RecordingMethodCaches,
    site: CacheSiteId,
    dispatch: vela_bytecode::MethodDispatchHandle,
    method_id: MethodId,
    target: StandardMethodInlineCacheTarget,
) {
    let entry = caches
        .entry(site)
        .expect("standard set cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard set cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Set);
    assert_eq!(standard_method.target, target);
}

fn linked_set_no_arg_cache_program(method: &str, receiver_values: &[i64]) -> LinkedSetCacheFixture {
    let method_id = vela_stdlib::std_method_id("Set", method).expect("Set method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name(method);
    let set_from_array_name = program.intern_debug_name("set::from_array");
    let set_from_array = program.push_native_function(vela_bytecode::LinkedNativeFunction::new(
        std_function_id(StdFunctionImplementation::SetFromArray),
        set_from_array_name,
    ));
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let receiver_array = Register(receiver_values.len() as u16);
    let receiver_set = Register(receiver_array.0 + 1);
    let result = Register(receiver_set.0 + 1);
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, result.0 + 1);

    push_i64_constants(&mut code, receiver_values, 0);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeArray {
            dst: receiver_array,
            elements: (0..receiver_values.len())
                .map(|index| Register(index as u16))
                .collect(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallNative {
            dst: Some(receiver_set),
            native: set_from_array,
            debug_name: set_from_array_name,
            cache_site: None,
            args: vec![receiver_array],
        },
    ));
    let site = code.push_cache_site(
        CacheSiteKind::MethodCall,
        InstructionOffset(code.instructions.len()),
    );
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: result,
            receiver: receiver_set,
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: Vec::new(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: result },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn linked_set_values_cache_program() -> LinkedSetCacheFixture {
    let method_id = vela_stdlib::std_method_id("Set", "values").expect("Set::values method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("values");
    let set_from_array_name = program.intern_debug_name("set::from_array");
    let set_from_array = program.push_native_function(vela_bytecode::LinkedNativeFunction::new(
        std_function_id(StdFunctionImplementation::SetFromArray),
        set_from_array_name,
    ));
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    push_i64_constants(&mut code, &[2, 4], 0);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeArray {
            dst: Register(2),
            elements: vec![Register(0), Register(1)],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallNative {
            dst: Some(Register(3)),
            native: set_from_array,
            debug_name: set_from_array_name,
            cache_site: None,
            args: vec![Register(2)],
        },
    ));
    let site = code.push_cache_site(
        CacheSiteKind::MethodCall,
        InstructionOffset(code.instructions.len()),
    );
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(4),
            receiver: Register(3),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: Vec::new(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(4) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn linked_set_mutator_cache_program(
    method: &str,
    receiver_values: &[i64],
    args: &[i64],
) -> LinkedSetCacheFixture {
    let method_id = vela_stdlib::std_method_id("Set", method).expect("Set method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name(method);
    let set_from_array_name = program.intern_debug_name("set::from_array");
    let set_from_array = program.push_native_function(vela_bytecode::LinkedNativeFunction::new(
        std_function_id(StdFunctionImplementation::SetFromArray),
        set_from_array_name,
    ));
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let receiver_array = Register(receiver_values.len() as u16);
    let arg_start = receiver_values.len() + 1;
    let receiver_set = Register((arg_start + args.len()) as u16);
    let result = Register(receiver_set.0 + 1);
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, result.0 + 1);

    push_i64_constants(&mut code, receiver_values, 0);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeArray {
            dst: receiver_array,
            elements: (0..receiver_values.len())
                .map(|index| Register(index as u16))
                .collect(),
        },
    ));
    push_i64_constants(&mut code, args, arg_start);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallNative {
            dst: Some(receiver_set),
            native: set_from_array,
            debug_name: set_from_array_name,
            cache_site: None,
            args: vec![receiver_array],
        },
    ));
    let site = code.push_cache_site(
        CacheSiteKind::MethodCall,
        InstructionOffset(code.instructions.len()),
    );
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: result,
            receiver: receiver_set,
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: (arg_start..arg_start + args.len())
                .map(|index| vela_bytecode::CallArgument::Register(Register(index as u16)))
                .collect(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: result },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn linked_set_combination_cache_program(
    method: &str,
    receiver_values: &[i64],
    other_values: &[i64],
) -> LinkedSetCacheFixture {
    linked_set_combination_cache_program_with_return(method, receiver_values, other_values, false)
}

fn linked_set_extend_return_receiver_cache_program(
    receiver_values: &[i64],
    other_values: &[i64],
) -> LinkedSetCacheFixture {
    linked_set_combination_cache_program_with_return("extend", receiver_values, other_values, true)
}

fn linked_set_combination_cache_program_with_return(
    method: &str,
    receiver_values: &[i64],
    other_values: &[i64],
    return_receiver: bool,
) -> LinkedSetCacheFixture {
    let method_id = vela_stdlib::std_method_id("Set", method).expect("Set method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name(method);
    let set_from_array_name = program.intern_debug_name("set::from_array");
    let set_from_array = program.push_native_function(vela_bytecode::LinkedNativeFunction::new(
        std_function_id(StdFunctionImplementation::SetFromArray),
        set_from_array_name,
    ));
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let receiver_array = Register(receiver_values.len() as u16);
    let other_start = receiver_values.len() + 1;
    let other_array = Register((other_start + other_values.len()) as u16);
    let receiver_set = Register(other_array.0 + 1);
    let other_set = Register(receiver_set.0 + 1);
    let result = Register(other_set.0 + 1);
    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, result.0 + 1);

    push_i64_constants(&mut code, receiver_values, 0);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeArray {
            dst: receiver_array,
            elements: (0..receiver_values.len())
                .map(|index| Register(index as u16))
                .collect(),
        },
    ));
    push_i64_constants(&mut code, other_values, other_start);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeArray {
            dst: other_array,
            elements: (other_start..other_start + other_values.len())
                .map(|index| Register(index as u16))
                .collect(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallNative {
            dst: Some(receiver_set),
            native: set_from_array,
            debug_name: set_from_array_name,
            cache_site: None,
            args: vec![receiver_array],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallNative {
            dst: Some(other_set),
            native: set_from_array,
            debug_name: set_from_array_name,
            cache_site: None,
            args: vec![other_array],
        },
    ));
    let site = code.push_cache_site(
        CacheSiteKind::MethodCall,
        InstructionOffset(code.instructions.len()),
    );
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: result,
            receiver: receiver_set,
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: vec![vela_bytecode::CallArgument::Register(other_set)],
        },
    ));
    let return_register = if return_receiver {
        receiver_set
    } else {
        result
    };
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return {
            src: return_register,
        },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn linked_set_self_extend_cache_program() -> LinkedSetCacheFixture {
    let method_id = vela_stdlib::std_method_id("Set", "extend").expect("Set::extend method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("extend");
    let set_from_array_name = program.intern_debug_name("set::from_array");
    let set_from_array = program.push_native_function(vela_bytecode::LinkedNativeFunction::new(
        std_function_id(StdFunctionImplementation::SetFromArray),
        set_from_array_name,
    ));
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    push_i64_constants(&mut code, &[2, 4], 0);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeArray {
            dst: Register(2),
            elements: vec![Register(0), Register(1)],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallNative {
            dst: Some(Register(3)),
            native: set_from_array,
            debug_name: set_from_array_name,
            cache_site: None,
            args: vec![Register(2)],
        },
    ));
    let site = code.push_cache_site(
        CacheSiteKind::MethodCall,
        InstructionOffset(code.instructions.len()),
    );
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(4),
            receiver: Register(3),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: vec![vela_bytecode::CallArgument::Register(Register(3))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(3) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn run_linked_set_cache_owned_program(
    program: &vela_bytecode::LinkedProgram,
    caches: &RecordingMethodCaches,
) -> VmResult<OwnedValue> {
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let result = run_linked_set_cache_program(program, caches, &mut heap_execution)?;
    crate::heap_values::value_to_owned(&result, Some(&heap_execution))
}

fn run_linked_set_cache_program(
    program: &vela_bytecode::LinkedProgram,
    caches: &RecordingMethodCaches,
    heap_execution: &mut HeapExecution<'_>,
) -> VmResult<RuntimeValue> {
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
        Some(heap_execution),
        Some(&mut budget),
    )
}

fn main_code(program: &vela_bytecode::LinkedProgram) -> &vela_bytecode::LinkedCodeObject {
    program
        .functions()
        .find(|(_, code)| program.debug_name(code.debug_name) == "main")
        .map(|(_, code)| code)
        .expect("linked set method cache fixture should have main")
}

fn std_function_id(implementation: StdFunctionImplementation) -> FunctionId {
    stdlib_function_runtime_bindings()
        .into_iter()
        .find_map(|binding| (binding.implementation == implementation).then_some(binding.id))
        .expect("standard function implementation should have a manifest id")
}

fn push_i64_constants(
    code: &mut vela_bytecode::LinkedCodeObject,
    values: &[i64],
    start_register: usize,
) {
    for (offset, value) in values.iter().copied().enumerate() {
        let constant = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(value)));
        code.push_instruction(vela_bytecode::linked::Instruction::new(
            vela_bytecode::linked::InstructionKind::LoadConst {
                dst: Register((start_register + offset) as u16),
                constant,
            },
        ));
    }
}
