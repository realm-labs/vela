use super::linked_standard_method_cache_fixtures::*;
use super::linked_standard_method_cache_support::*;
use super::*;
use crate::value::Value as RuntimeValue;

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
