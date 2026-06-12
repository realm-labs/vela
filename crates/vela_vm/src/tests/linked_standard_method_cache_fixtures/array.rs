use super::*;

pub(in crate::tests) fn linked_array_contains_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("contains", &[2, 4], 2, &[1])
}

pub(in crate::tests) fn linked_array_first_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("first", &[2, 4], 2, &[])
}

pub(in crate::tests) fn linked_array_last_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("last", &[2, 4], 2, &[])
}

pub(in crate::tests) fn linked_array_index_of_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("index_of", &[2, 4, 4], 2, &[2])
}

pub(in crate::tests) fn linked_array_slice_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("slice", &[2, 4, 6, 1, 3], 3, &[3, 4])
}

pub(in crate::tests) fn linked_array_push_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("push", &[2, 4], 1, &[1])
}

pub(in crate::tests) fn linked_array_pop_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("pop", &[2, 4], 2, &[])
}

pub(in crate::tests) fn linked_array_insert_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("insert", &[2, 1, 9], 1, &[1, 2])
}

pub(in crate::tests) fn linked_array_remove_at_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("remove_at", &[2, 4, 1], 2, &[2])
}

pub(in crate::tests) fn linked_array_clear_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("clear", &[2, 4], 2, &[])
}

pub(in crate::tests) fn linked_array_reverse_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("reverse", &[2, 4, 6], 3, &[])
}

pub(in crate::tests) fn linked_array_distinct_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("distinct", &[2, 4, 2], 3, &[])
}

pub(in crate::tests) fn linked_array_join_cache_program() -> LinkedMethodCacheFixture {
    let method_id = vela_stdlib::std_method_id("Array", "join").expect("Array::join method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("join");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    let first = code.push_constant(Constant::String("raid".into()));
    let second = code.push_constant(Constant::String("quest".into()));
    let separator = code.push_constant(Constant::String(",".into()));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: first,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(1),
            constant: second,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(2),
            constant: separator,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeArray {
            dst: Register(3),
            elements: vec![Register(0), Register(1)],
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
            args: vec![vela_bytecode::CallArgument::Register(Register(2))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(4) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

pub(in crate::tests) fn linked_array_sort_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("sort", &[4, 2, 6], 3, &[])
}

pub(in crate::tests) fn linked_array_min_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("min", &[4, 2, 6], 3, &[])
}

pub(in crate::tests) fn linked_array_max_cache_program() -> LinkedMethodCacheFixture {
    linked_array_i64_call_cache_program("max", &[4, 2, 6], 3, &[])
}

fn linked_array_i64_call_cache_program(
    method: &str,
    values: &[i64],
    array_element_count: usize,
    arg_registers: &[usize],
) -> LinkedMethodCacheFixture {
    assert!(
        array_element_count <= values.len(),
        "array fixture cannot use more elements than constants"
    );
    let method_id = vela_stdlib::std_method_id("Array", method).expect("Array method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name(method);
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let receiver = register(values.len());
    let dst = register(values.len() + 1);
    let mut code = vela_bytecode::LinkedCodeObject::new(
        main_name,
        u16::try_from(values.len() + 2).expect("array cache fixture register count fits u16"),
    );
    push_i64_constants(&mut code, values, 0);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeArray {
            dst: receiver,
            elements: (0..array_element_count).map(register).collect(),
        },
    ));
    let site = code.push_cache_site(
        CacheSiteKind::MethodCall,
        InstructionOffset(code.instructions.len()),
    );
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst,
            receiver,
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: arg_registers
                .iter()
                .copied()
                .map(register)
                .map(vela_bytecode::CallArgument::Register)
                .collect(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: dst },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn register(index: usize) -> Register {
    Register(u16::try_from(index).expect("array cache fixture register fits u16"))
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
