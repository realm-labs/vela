use super::linked_standard_method_cache_fixtures::*;
use super::linked_standard_method_cache_support::*;
use super::*;
use crate::value::Value as RuntimeValue;
use vela_stdlib_runtime::{StdFunctionImplementation, stdlib_function_runtime_bindings};

#[test]
fn linked_standard_value_method_caches_is_empty_targets() {
    assert_no_arg_bool_cache(
        linked_string_no_arg_cache_program("is_empty", ""),
        StandardMethodReceiver::String,
        StandardMethodInlineCacheTarget::IsEmpty,
        true,
    );
    assert_no_arg_bool_cache(
        linked_bytes_no_arg_cache_program("is_empty", Vec::new()),
        StandardMethodReceiver::Bytes,
        StandardMethodInlineCacheTarget::IsEmpty,
        true,
    );
    assert_no_arg_bool_cache(
        linked_range_no_arg_cache_program("is_empty", 3, 3),
        StandardMethodReceiver::Range,
        StandardMethodInlineCacheTarget::IsEmpty,
        true,
    );
}

#[test]
fn linked_standard_value_method_caches_len_targets() {
    assert_no_arg_i64_cache(
        linked_bytes_no_arg_cache_program("len", vec![1, 2, 3]),
        StandardMethodReceiver::Bytes,
        StandardMethodInlineCacheTarget::Len,
        3,
        false,
    );
    assert_no_arg_i64_cache(
        linked_range_no_arg_cache_program("len", 2, 7),
        StandardMethodReceiver::Range,
        StandardMethodInlineCacheTarget::Len,
        5,
        false,
    );
    assert_no_arg_i64_cache(
        linked_array_len_cache_program(),
        StandardMethodReceiver::Array,
        StandardMethodInlineCacheTarget::Len,
        2,
        false,
    );
    assert_no_arg_i64_cache(
        linked_map_len_cache_program(),
        StandardMethodReceiver::Map,
        StandardMethodInlineCacheTarget::Len,
        2,
        false,
    );
    assert_no_arg_i64_cache(
        linked_set_len_cache_program(),
        StandardMethodReceiver::Set,
        StandardMethodInlineCacheTarget::Len,
        2,
        true,
    );
}

fn assert_no_arg_bool_cache(
    fixture: LinkedMethodCacheFixture,
    receiver: StandardMethodReceiver,
    target: StandardMethodInlineCacheTarget,
    expected: bool,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(RuntimeValue::Bool(expected));

    assert_eq!(run_linked_method_cache_program(&program, &caches), expected);
    let entry = caches
        .entry(site)
        .expect("standard no-arg bool cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard no-arg bool cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, receiver);
    assert_eq!(standard_method.target, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(run_linked_method_cache_program(&program, &caches), expected);
    assert_eq!(caches.set_count(), 2);
}

fn assert_no_arg_i64_cache(
    fixture: LinkedMethodCacheFixture,
    receiver: StandardMethodReceiver,
    target: StandardMethodInlineCacheTarget,
    expected: i64,
    with_standard_natives: bool,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(RuntimeValue::i64(expected));
    let run = |program, caches| {
        if with_standard_natives {
            run_linked_method_cache_program_with_standard_natives(program, caches)
        } else {
            run_linked_method_cache_program(program, caches)
        }
    };

    assert_eq!(run(&program, &caches), expected);
    let entry = caches
        .entry(site)
        .expect("standard no-arg i64 cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard no-arg i64 cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, receiver);
    assert_eq!(standard_method.target, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(run(&program, &caches), expected);
    assert_eq!(caches.set_count(), 2);
}

fn linked_bytes_no_arg_cache_program(method: &str, receiver: Vec<u8>) -> LinkedMethodCacheFixture {
    let method_id = vela_stdlib::std_method_id("Bytes", method).expect("Bytes method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name(method);
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 2);
    let receiver = code.push_constant(Constant::Bytes(receiver));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: receiver,
        },
    ));
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(1));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(1),
            receiver: Register(0),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: Vec::new(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(1) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn linked_array_len_cache_program() -> LinkedMethodCacheFixture {
    let method_id = vela_stdlib::std_method_id("Array", "len").expect("Array::len method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("len");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 4);
    load_i64(&mut code, Register(0), 2);
    load_i64(&mut code, Register(1), 4);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeArray {
            dst: Register(2),
            elements: vec![Register(0), Register(1)],
        },
    ));
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(3));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(3),
            receiver: Register(2),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: Vec::new(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(3) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn linked_map_len_cache_program() -> LinkedMethodCacheFixture {
    let method_id = vela_stdlib::std_method_id("Map", "len").expect("Map::len method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("len");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 4);
    let gold = code.push_constant(Constant::String("gold".into()));
    let xp = code.push_constant(Constant::String("xp".into()));
    load_i64(&mut code, Register(0), 4);
    load_i64(&mut code, Register(1), 8);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeMap {
            dst: Register(2),
            entries: vec![(gold, Register(0)), (xp, Register(1))],
        },
    ));
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(3));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(3),
            receiver: Register(2),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: Vec::new(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(3) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn linked_set_len_cache_program() -> LinkedMethodCacheFixture {
    let method_id = vela_stdlib::std_method_id("Set", "len").expect("Set::len method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("len");
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
    load_i64(&mut code, Register(0), 2);
    load_i64(&mut code, Register(1), 4);
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
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(4));
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

fn load_i64(code: &mut vela_bytecode::LinkedCodeObject, dst: Register, value: i64) {
    let constant = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(value)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst { dst, constant },
    ));
}

fn std_function_id(implementation: StdFunctionImplementation) -> vela_def::FunctionId {
    stdlib_function_runtime_bindings()
        .into_iter()
        .find_map(|binding| (binding.implementation == implementation).then_some(binding.id))
        .expect("standard function implementation should have a manifest id")
}

fn linked_range_no_arg_cache_program(
    method: &str,
    start: i64,
    end: i64,
) -> LinkedMethodCacheFixture {
    let method_id = vela_stdlib::std_method_id("Range", method).expect("Range method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name(method);
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 4);
    let start = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(start)));
    let end = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(end)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: start,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(1),
            constant: end,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeRange {
            dst: Register(2),
            start: Register(0),
            end: Register(1),
            inclusive: false,
        },
    ));
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(3));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(3),
            receiver: Register(2),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: Vec::new(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(3) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}
