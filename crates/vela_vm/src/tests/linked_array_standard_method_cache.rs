use super::linked_standard_method_cache_fixtures::*;
use super::linked_standard_method_cache_support::*;
use super::*;
use crate::value::Value as RuntimeValue;

#[test]
fn linked_standard_value_method_caches_array_contains_target() {
    assert_array_bool_cache(
        linked_array_contains_cache_program(),
        StandardMethodInlineCacheTarget::Contains,
        true,
    );
}

#[test]
fn linked_standard_value_method_caches_array_is_empty_target() {
    assert_array_bool_cache(
        linked_array_is_empty_cache_program(),
        StandardMethodInlineCacheTarget::IsEmpty,
        false,
    );
}

#[test]
fn linked_standard_value_method_caches_array_first_target() {
    assert_array_option_scalar_cache(
        linked_array_first_cache_program(),
        StandardMethodInlineCacheTarget::First,
        2,
    );
}

#[test]
fn linked_standard_value_method_caches_array_last_target() {
    assert_array_option_scalar_cache(
        linked_array_last_cache_program(),
        StandardMethodInlineCacheTarget::Last,
        4,
    );
}

#[test]
fn linked_standard_value_method_caches_array_index_of_target() {
    assert_array_option_scalar_cache(
        linked_array_index_of_cache_program(),
        StandardMethodInlineCacheTarget::IndexOf,
        1,
    );
}

#[test]
fn linked_standard_value_method_caches_array_slice_target() {
    assert_array_owned_cache(
        linked_array_slice_cache_program(),
        StandardMethodInlineCacheTarget::Slice,
        OwnedValue::Array(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(6)),
        ]),
    );
}

#[test]
fn linked_standard_value_method_caches_array_push_target() {
    assert_array_owned_cache(
        linked_array_push_cache_program(),
        StandardMethodInlineCacheTarget::Push,
        OwnedValue::Null,
    );
}

#[test]
fn linked_standard_value_method_caches_array_pop_target() {
    assert_array_option_scalar_cache(
        linked_array_pop_cache_program(),
        StandardMethodInlineCacheTarget::Pop,
        4,
    );
}

#[test]
fn linked_standard_value_method_caches_array_insert_target() {
    assert_array_owned_cache(
        linked_array_insert_cache_program(),
        StandardMethodInlineCacheTarget::Insert,
        OwnedValue::Null,
    );
}

#[test]
fn linked_standard_value_method_caches_array_remove_at_target() {
    assert_array_option_scalar_cache(
        linked_array_remove_at_cache_program(),
        StandardMethodInlineCacheTarget::RemoveAt,
        4,
    );
}

#[test]
fn linked_standard_value_method_caches_array_clear_target() {
    assert_array_owned_cache(
        linked_array_clear_cache_program(),
        StandardMethodInlineCacheTarget::Clear,
        OwnedValue::Null,
    );
}

#[test]
fn linked_standard_value_method_caches_array_extend_target() {
    assert_array_owned_cache(
        linked_array_extend_cache_program(),
        StandardMethodInlineCacheTarget::Extend,
        OwnedValue::Null,
    );
}

#[test]
fn linked_standard_value_method_caches_array_single_extend_target() {
    let (program, site, dispatch, method_id) = linked_array_single_extend_cache_program();
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(OwnedValue::Array(vec![
        OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
        OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
    ]));

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_array_cache_entry(
        &caches,
        site,
        dispatch,
        method_id,
        StandardMethodInlineCacheTarget::Extend,
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_array_self_extend_target() {
    let (program, site, dispatch, method_id) = linked_array_self_extend_cache_program();
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(OwnedValue::Array(vec![
        OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
        OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
        OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
        OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
    ]));

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_array_cache_entry(
        &caches,
        site,
        dispatch,
        method_id,
        StandardMethodInlineCacheTarget::Extend,
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_array_reverse_target() {
    assert_array_owned_cache(
        linked_array_reverse_cache_program(),
        StandardMethodInlineCacheTarget::Reverse,
        OwnedValue::Array(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(6)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
        ]),
    );
}

#[test]
fn linked_standard_value_method_caches_array_distinct_target() {
    assert_array_owned_cache(
        linked_array_distinct_cache_program(),
        StandardMethodInlineCacheTarget::Distinct,
        OwnedValue::Array(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
        ]),
    );
}

#[test]
fn linked_standard_value_method_caches_array_join_target() {
    assert_array_owned_cache(
        linked_array_join_cache_program(),
        StandardMethodInlineCacheTarget::Join,
        OwnedValue::String("raid,quest".to_owned()),
    );
}

#[test]
fn linked_standard_value_method_caches_array_sort_target() {
    assert_array_owned_cache(
        linked_array_sort_cache_program(),
        StandardMethodInlineCacheTarget::Sort,
        OwnedValue::Array(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(6)),
        ]),
    );
}

#[test]
fn linked_standard_value_method_caches_array_extrema_targets() {
    assert_array_option_scalar_cache(
        linked_array_min_cache_program(),
        StandardMethodInlineCacheTarget::Min,
        2,
    );
    assert_array_option_scalar_cache(
        linked_array_max_cache_program(),
        StandardMethodInlineCacheTarget::Max,
        6,
    );
}

#[test]
fn linked_standard_value_method_caches_array_sum_target() {
    assert_array_scalar_cache(
        linked_array_sum_cache_program(),
        StandardMethodInlineCacheTarget::Sum,
        12,
    );
}

#[test]
fn linked_standard_value_method_caches_array_values_target() {
    assert_array_owned_cache(
        linked_array_values_collect_cache_program(),
        StandardMethodInlineCacheTarget::Values,
        OwnedValue::Array(vec![
            OwnedValue::Scalar(vela_common::ScalarValue::I64(2)),
            OwnedValue::Scalar(vela_common::ScalarValue::I64(4)),
        ]),
    );
}

fn assert_array_bool_cache(
    fixture: LinkedMethodCacheFixture,
    target: StandardMethodInlineCacheTarget,
    expected: bool,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(RuntimeValue::Bool(expected));

    assert_eq!(run_linked_method_cache_program(&program, &caches), expected);
    assert_array_cache_entry(&caches, site, dispatch, method_id, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(run_linked_method_cache_program(&program, &caches), expected);
    assert_eq!(caches.set_count(), 2);
}

fn assert_array_scalar_cache(
    fixture: LinkedMethodCacheFixture,
    target: StandardMethodInlineCacheTarget,
    expected: i64,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(RuntimeValue::i64(expected));

    assert_eq!(run_linked_method_cache_program(&program, &caches), expected);
    assert_array_cache_entry(&caches, site, dispatch, method_id, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(run_linked_method_cache_program(&program, &caches), expected);
    assert_eq!(caches.set_count(), 2);
}

fn assert_array_option_scalar_cache(
    fixture: LinkedMethodCacheFixture,
    target: StandardMethodInlineCacheTarget,
    expected: i64,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(owned_option_some(OwnedValue::Scalar(
        vela_common::ScalarValue::I64(expected),
    )));

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_array_cache_entry(&caches, site, dispatch, method_id, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_eq!(caches.set_count(), 2);
}

fn assert_array_owned_cache(
    fixture: LinkedMethodCacheFixture,
    target: StandardMethodInlineCacheTarget,
    expected: OwnedValue,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(expected);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_array_cache_entry(&caches, site, dispatch, method_id, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_eq!(caches.set_count(), 2);
}

fn assert_array_cache_entry(
    caches: &RecordingMethodCaches,
    site: CacheSiteId,
    dispatch: vela_bytecode::MethodDispatchHandle,
    method_id: MethodId,
    target: StandardMethodInlineCacheTarget,
) {
    let entry = caches
        .entry(site)
        .expect("standard array cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard array cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Array);
    assert_eq!(standard_method.target, target);
}

fn linked_array_extend_cache_program() -> LinkedMethodCacheFixture {
    linked_array_extend_cache_program_with_options(false, false)
}

fn linked_array_single_extend_cache_program() -> LinkedMethodCacheFixture {
    linked_array_extend_cache_program_with_options(true, true)
}

fn linked_array_extend_cache_program_with_options(
    single_extension: bool,
    return_receiver: bool,
) -> LinkedMethodCacheFixture {
    let method_id = vela_stdlib::std_method_id("Array", "extend").expect("Array::extend method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("extend");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 6);
    let first = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let second = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    let third = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(6)));
    for (index, constant) in [first, second, third].into_iter().enumerate() {
        code.push_instruction(vela_bytecode::linked::Instruction::new(
            vela_bytecode::linked::InstructionKind::LoadConst {
                dst: Register(index as u16),
                constant,
            },
        ));
    }
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeArray {
            dst: Register(3),
            elements: vec![Register(0)],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeArray {
            dst: Register(4),
            elements: if single_extension {
                vec![Register(1)]
            } else {
                vec![Register(1), Register(2)]
            },
        },
    ));
    let site = code.push_cache_site(
        CacheSiteKind::MethodCall,
        InstructionOffset(code.instructions.len()),
    );
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(5),
            receiver: Register(3),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: vec![vela_bytecode::CallArgument::Register(Register(4))],
        },
    ));
    let return_register = if return_receiver {
        Register(3)
    } else {
        Register(5)
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

fn linked_array_self_extend_cache_program() -> LinkedMethodCacheFixture {
    let method_id = vela_stdlib::std_method_id("Array", "extend").expect("Array::extend method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("extend");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    let first = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(2)));
    let second = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    for (index, constant) in [first, second].into_iter().enumerate() {
        code.push_instruction(vela_bytecode::linked::Instruction::new(
            vela_bytecode::linked::InstructionKind::LoadConst {
                dst: Register(index as u16),
                constant,
            },
        ));
    }
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeArray {
            dst: Register(2),
            elements: vec![Register(0), Register(1)],
        },
    ));
    let site = code.push_cache_site(
        CacheSiteKind::MethodCall,
        InstructionOffset(code.instructions.len()),
    );
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(3),
            receiver: Register(2),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: vec![vela_bytecode::CallArgument::Register(Register(2))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}
