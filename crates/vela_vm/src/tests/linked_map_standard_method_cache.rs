use super::linked_standard_method_cache_support::*;
use super::*;
use crate::budget::CollectionLimits;

type LinkedMapCacheFixture = (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    MethodId,
);

#[test]
fn linked_standard_value_method_caches_map_keys_target() {
    assert_map_owned_cache(
        linked_map_view_collect_cache_program("keys"),
        StandardMethodInlineCacheTarget::Keys,
        OwnedValue::array(["gold", "xp"]),
    );
}

#[test]
fn linked_standard_value_method_caches_map_is_empty_target() {
    assert_map_bool_cache(
        linked_map_no_arg_cache_program("is_empty"),
        StandardMethodInlineCacheTarget::IsEmpty,
        false,
    );
}

#[test]
fn linked_standard_value_method_caches_map_values_target() {
    assert_map_owned_cache(
        linked_map_view_collect_cache_program("values"),
        StandardMethodInlineCacheTarget::Values,
        OwnedValue::Array(vec![OwnedValue::i64(4), OwnedValue::i64(8)]),
    );
}

#[test]
fn linked_standard_value_method_caches_map_entries_target() {
    assert_map_owned_cache(
        linked_map_view_collect_cache_program("entries"),
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

#[test]
fn linked_cached_map_merge_limit_counts_unique_value_keys() {
    let (program, site, dispatch, method_id) = linked_map_merge_cache_program();
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(OwnedValue::map([
        ("gold", OwnedValue::i64(4)),
        ("quest", OwnedValue::i64(8)),
        ("xp", OwnedValue::i64(10)),
    ]));

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_map_cache_entry(
        &caches,
        site,
        dispatch,
        method_id,
        StandardMethodInlineCacheTarget::Merge,
    );

    let mut budget = ExecutionBudget::unbounded().with_collection_limits(CollectionLimits {
        max_array_len: usize::MAX,
        max_map_entries: 3,
        max_set_len: usize::MAX,
    });

    assert_eq!(
        run_linked_method_cache_owned_program_with_budget(&program, &caches, &mut budget),
        expected
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_map_set_target() {
    assert_map_owned_cache(
        linked_map_set_cache_program(),
        StandardMethodInlineCacheTarget::Set,
        OwnedValue::i64(8),
    );
}

#[test]
fn linked_standard_value_method_caches_map_remove_target() {
    assert_map_owned_cache(
        linked_map_remove_cache_program(),
        StandardMethodInlineCacheTarget::Remove,
        owned_option_some(OwnedValue::i64(8)),
    );
}

#[test]
fn linked_standard_value_method_caches_map_clear_target() {
    assert_map_owned_cache(
        linked_map_no_arg_cache_program("clear"),
        StandardMethodInlineCacheTarget::Clear,
        OwnedValue::Null,
    );
}

#[test]
fn linked_standard_value_method_caches_map_extend_target() {
    assert_map_owned_cache(
        linked_map_extend_cache_program(),
        StandardMethodInlineCacheTarget::Extend,
        OwnedValue::Null,
    );
}

#[test]
fn linked_standard_value_method_caches_map_single_extend_target() {
    let (program, site, dispatch, method_id) = linked_map_extend_return_receiver_cache_program();
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(OwnedValue::map([
        ("gold", OwnedValue::i64(4)),
        ("xp", OwnedValue::i64(8)),
    ]));

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_map_cache_entry(
        &caches,
        site,
        dispatch,
        method_id,
        StandardMethodInlineCacheTarget::Extend,
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_map_self_extend_target() {
    let (program, site, dispatch, method_id) = linked_map_self_extend_cache_program();
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(OwnedValue::map([("gold", OwnedValue::i64(4))]));

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_map_cache_entry(
        &caches,
        site,
        dispatch,
        method_id,
        StandardMethodInlineCacheTarget::Extend,
    );
    assert_eq!(caches.set_count(), 2);
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

fn assert_map_bool_cache(
    fixture: LinkedMapCacheFixture,
    target: StandardMethodInlineCacheTarget,
    expected: bool,
) {
    let (program, site, dispatch, method_id) = fixture;
    let caches = RecordingMethodCaches::new(1);
    let expected = Ok(crate::value::Value::Bool(expected));

    assert_eq!(run_linked_method_cache_program(&program, &caches), expected);
    assert_map_cache_entry(&caches, site, dispatch, method_id, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(run_linked_method_cache_program(&program, &caches), expected);
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

fn linked_map_view_collect_cache_program(method: &str) -> LinkedMapCacheFixture {
    let method_id = vela_stdlib::std_method_id("Map", method).expect("Map view method id");
    let collect_method_id = vela_stdlib::std_method_id("Iterator", "collect_array")
        .expect("Iterator::collect_array method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name(method);
    let collect_name = program.intern_debug_name("collect_array");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));
    let collect_dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        collect_name,
        vela_bytecode::LinkedMethodDispatchKind::Value {
            method_id: collect_method_id,
        },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
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
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(4),
            receiver: Register(3),
            dispatch: collect_dispatch,
            debug_name: collect_name,
            cache_site: None,
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

fn linked_map_set_cache_program() -> LinkedMapCacheFixture {
    let method_id = vela_stdlib::std_method_id("Map", "set").expect("Map::Set method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("set");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    let gold = code.push_constant(Constant::String("gold".into()));
    let xp = code.push_constant(Constant::String("xp".into()));
    load_i64(&mut code, Register(0), 4);
    load_i64(&mut code, Register(1), 8);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeMap {
            dst: Register(2),
            entries: vec![(gold, Register(0))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(3),
            constant: xp,
        },
    ));
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(4));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(4),
            receiver: Register(2),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: vec![
                vela_bytecode::CallArgument::Register(Register(3)),
                vela_bytecode::CallArgument::Register(Register(1)),
            ],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(4) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn linked_map_remove_cache_program() -> LinkedMapCacheFixture {
    let method_id = vela_stdlib::std_method_id("Map", "remove").expect("Map::remove method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("remove");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 4);
    let xp = code.push_constant(Constant::String("xp".into()));
    load_i64(&mut code, Register(0), 8);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeMap {
            dst: Register(1),
            entries: vec![(xp, Register(0))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(2),
            constant: xp,
        },
    ));
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(3));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(3),
            receiver: Register(1),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: vec![vela_bytecode::CallArgument::Register(Register(2))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(3) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn linked_map_extend_cache_program() -> LinkedMapCacheFixture {
    linked_map_extend_cache_program_with_return(false)
}

fn linked_map_extend_return_receiver_cache_program() -> LinkedMapCacheFixture {
    linked_map_extend_cache_program_with_return(true)
}

fn linked_map_extend_cache_program_with_return(return_receiver: bool) -> LinkedMapCacheFixture {
    let method_id = vela_stdlib::std_method_id("Map", "extend").expect("Map::extend method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("extend");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    let gold = code.push_constant(Constant::String("gold".into()));
    let xp = code.push_constant(Constant::String("xp".into()));
    load_i64(&mut code, Register(0), 4);
    load_i64(&mut code, Register(1), 8);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeMap {
            dst: Register(2),
            entries: vec![(gold, Register(0))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeMap {
            dst: Register(3),
            entries: vec![(xp, Register(1))],
        },
    ));
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(4));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(4),
            receiver: Register(2),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: vec![vela_bytecode::CallArgument::Register(Register(3))],
        },
    ));
    let return_register = if return_receiver {
        Register(2)
    } else {
        Register(4)
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

fn linked_map_self_extend_cache_program() -> LinkedMapCacheFixture {
    let method_id = vela_stdlib::std_method_id("Map", "extend").expect("Map::extend method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("extend");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 4);
    let gold = code.push_constant(Constant::String("gold".into()));
    load_i64(&mut code, Register(0), 4);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeMap {
            dst: Register(1),
            entries: vec![(gold, Register(0))],
        },
    ));
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(2));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(2),
            receiver: Register(1),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(1) },
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
