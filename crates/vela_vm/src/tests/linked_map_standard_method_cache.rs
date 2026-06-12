use super::linked_standard_method_cache_support::*;
use super::*;

type LinkedMapCacheFixture = (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    MethodId,
);

#[test]
fn linked_standard_value_method_caches_map_keys_target() {
    assert_map_owned_cache(
        linked_map_no_arg_cache_program("keys"),
        StandardMethodInlineCacheTarget::Keys,
        OwnedValue::array(["gold", "xp"]),
    );
}

#[test]
fn linked_standard_value_method_caches_map_values_target() {
    assert_map_owned_cache(
        linked_map_no_arg_cache_program("values"),
        StandardMethodInlineCacheTarget::Values,
        OwnedValue::Array(vec![OwnedValue::i64(4), OwnedValue::i64(8)]),
    );
}

#[test]
fn linked_standard_value_method_caches_map_entries_target() {
    assert_map_owned_cache(
        linked_map_no_arg_cache_program("entries"),
        StandardMethodInlineCacheTarget::Entries,
        OwnedValue::Array(vec![
            OwnedValue::record(
                "MapEntry",
                [
                    ("key", OwnedValue::String("gold".to_owned())),
                    ("value", OwnedValue::i64(4)),
                ],
            ),
            OwnedValue::record(
                "MapEntry",
                [
                    ("key", OwnedValue::String("xp".to_owned())),
                    ("value", OwnedValue::i64(8)),
                ],
            ),
        ]),
    );
}

#[test]
fn linked_standard_value_method_caches_map_merge_target() {
    assert_map_owned_cache(
        linked_map_merge_cache_program(),
        StandardMethodInlineCacheTarget::Merge,
        OwnedValue::map([
            ("gold", OwnedValue::i64(4)),
            ("quest", OwnedValue::i64(8)),
            ("xp", OwnedValue::i64(10)),
        ]),
    );
}

fn assert_map_owned_cache(
    fixture: LinkedMapCacheFixture,
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
    assert_map_cache_entry(&caches, site, dispatch, method_id, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_eq!(caches.set_count(), 2);
}

fn assert_map_cache_entry(
    caches: &RecordingMethodCaches,
    site: CacheSiteId,
    dispatch: vela_bytecode::MethodDispatchHandle,
    method_id: MethodId,
    target: StandardMethodInlineCacheTarget,
) {
    let entry = caches
        .entry(site)
        .expect("standard map cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard map cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Map);
    assert_eq!(standard_method.target, target);
}

fn linked_map_no_arg_cache_program(method: &str) -> LinkedMapCacheFixture {
    let method_id = vela_stdlib::std_method_id("Map", method).expect("Map method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name(method);
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

fn linked_map_merge_cache_program() -> LinkedMapCacheFixture {
    let method_id = vela_stdlib::std_method_id("Map", "merge").expect("Map::merge method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("merge");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 7);
    let gold = code.push_constant(Constant::String("gold".into()));
    let quest = code.push_constant(Constant::String("quest".into()));
    let xp = code.push_constant(Constant::String("xp".into()));
    load_i64(&mut code, Register(0), 4);
    load_i64(&mut code, Register(1), 6);
    load_i64(&mut code, Register(2), 8);
    load_i64(&mut code, Register(3), 10);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeMap {
            dst: Register(4),
            entries: vec![(gold, Register(0)), (xp, Register(1))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeMap {
            dst: Register(5),
            entries: vec![(quest, Register(2)), (xp, Register(3))],
        },
    ));
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(6));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(6),
            receiver: Register(4),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: vec![vela_bytecode::CallArgument::Register(Register(5))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(6) },
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
