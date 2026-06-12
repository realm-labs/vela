use super::*;
use crate::value::Value as RuntimeValue;
use std::cell::{Cell, RefCell};

#[test]
fn linked_standard_value_method_populates_readonly_inline_cache() {
    let (program, site, dispatch, method_id) = linked_standard_len_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(4))
    );
    let entry = caches
        .entry(site)
        .expect("standard method cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    assert_eq!(caches.set_count(), 2);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard method cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(standard_method.target, StandardMethodInlineCacheTarget::Len);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(4))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_refreshes_wrong_receiver_guard() {
    let (program, site, dispatch, method_id) = linked_standard_len_cache_program();
    let caches = RecordingMethodCaches::new(1);
    let debug_name = program
        .method_dispatch(dispatch)
        .expect("dispatch should exist")
        .debug_name;
    caches.prime(
        site,
        MethodInlineCacheEntry {
            dispatch,
            debug_name,
            target: MethodInlineCacheTarget::Value {
                method_id,
                standard_method: Some(StandardMethodInlineCacheEntry {
                    receiver: StandardMethodReceiver::Array,
                    target: StandardMethodInlineCacheTarget::Len,
                }),
            },
        },
    );

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(4))
    );
    let entry = caches
        .entry(site)
        .expect("standard method cache should refresh");
    let MethodInlineCacheTarget::Value {
        standard_method: Some(standard_method),
        ..
    } = entry.target
    else {
        panic!("standard method cache should store refreshed value target");
    };
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(standard_method.target, StandardMethodInlineCacheTarget::Len);
    assert_eq!(caches.set_count(), 1);
}

#[test]
fn linked_standard_value_method_caches_predicate_target() {
    let (program, site, dispatch, method_id) = linked_string_contains_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Bool(true))
    );
    let entry = caches
        .entry(site)
        .expect("standard predicate cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard predicate cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::String);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::Contains
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Bool(true))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_bytes_accessor_target() {
    let (program, site, dispatch, method_id) = linked_bytes_get_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Scalar(vela_common::ScalarValue::U8(21)))
    );
    let entry = caches
        .entry(site)
        .expect("standard bytes accessor cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard bytes accessor cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Bytes);
    assert_eq!(standard_method.target, StandardMethodInlineCacheTarget::Get);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Scalar(vela_common::ScalarValue::U8(21)))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_option_predicate_target() {
    let (program, site, dispatch, method_id) = linked_option_is_some_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Bool(true))
    );
    let entry = caches
        .entry(site)
        .expect("standard option cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard option cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Option);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::IsSome
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::Bool(true))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_result_unwrap_or_target() {
    let (program, site, dispatch, method_id) = linked_result_unwrap_or_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(17))
    );
    let entry = caches
        .entry(site)
        .expect("standard result cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard result cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Result);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::UnwrapOr
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(17))
    );
    assert_eq!(caches.set_count(), 2);
}

#[test]
fn linked_standard_value_method_caches_map_get_or_target() {
    let (program, site, dispatch, method_id) = linked_map_get_or_cache_program();
    let caches = RecordingMethodCaches::new(1);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(8))
    );
    let entry = caches
        .entry(site)
        .expect("standard map get_or cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard map get_or cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, StandardMethodReceiver::Map);
    assert_eq!(
        standard_method.target,
        StandardMethodInlineCacheTarget::GetOr
    );
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_program(&program, &caches),
        Ok(RuntimeValue::i64(8))
    );
    assert_eq!(caches.set_count(), 2);
}

fn linked_standard_len_cache_program() -> (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    vela_def::MethodId,
) {
    let method_id = vela_stdlib::std_method_id("String", "len").expect("String::len method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("len");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 2);
    let value = code.push_constant(Constant::String("gold".into()));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
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

fn linked_map_get_or_cache_program() -> (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    vela_def::MethodId,
) {
    let method_id = vela_stdlib::std_method_id("Map", "get_or").expect("Map::get_or method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("get_or");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 5);
    let key = code.push_constant(Constant::String("xp".into()));
    let value = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(8)));
    let fallback = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(99)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: value,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeMap {
            dst: Register(1),
            entries: vec![(key, Register(0))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(2),
            constant: key,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(3),
            constant: fallback,
        },
    ));
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(4));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(4),
            receiver: Register(1),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: vec![
                vela_bytecode::CallArgument::Register(Register(2)),
                vela_bytecode::CallArgument::Register(Register(3)),
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

fn linked_option_is_some_cache_program() -> (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    vela_def::MethodId,
) {
    let method_id =
        vela_stdlib::std_method_id("Option", "is_some").expect("Option::is_some method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("is_some");
    let field_name = program.intern_debug_name("0");
    let option_type = push_standard_type(&mut program, "Option");
    let some_variant = push_standard_variant(&mut program, option_type, "Option", "Some");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 3);
    let payload = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(4)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: payload,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeEnum {
            dst: Register(1),
            enum_ty: option_type,
            variant: some_variant,
            fields: vec![(vela_bytecode::FieldSlot::new(0), field_name, Register(0))],
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
            args: Vec::new(),
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn linked_result_unwrap_or_cache_program() -> (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    vela_def::MethodId,
) {
    let method_id =
        vela_stdlib::std_method_id("Result", "unwrap_or").expect("Result::unwrap_or method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("unwrap_or");
    let field_name = program.intern_debug_name("0");
    let result_type = push_standard_type(&mut program, "Result");
    let err_variant = push_standard_variant(&mut program, result_type, "Result", "Err");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 4);
    let payload = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(404)));
    let fallback = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(17)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: payload,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(1),
            constant: fallback,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeEnum {
            dst: Register(2),
            enum_ty: result_type,
            variant: err_variant,
            fields: vec![(vela_bytecode::FieldSlot::new(0), field_name, Register(0))],
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
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(3) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn push_standard_type(
    program: &mut vela_bytecode::LinkedProgram,
    name: &str,
) -> vela_bytecode::TypeHandle {
    let debug_name = program.intern_debug_name(name);
    let type_id = vela_stdlib::std_type_id(name).expect("standard type id should exist");
    program.push_type(vela_bytecode::LinkedType::new(type_id, debug_name))
}

fn push_standard_variant(
    program: &mut vela_bytecode::LinkedProgram,
    ty: vela_bytecode::TypeHandle,
    enum_name: &str,
    variant_name: &str,
) -> vela_bytecode::VariantHandle {
    let debug_name = program.intern_debug_name(format!("{enum_name}::{variant_name}"));
    let variant_id = vela_stdlib::std_variant_id(enum_name, variant_name)
        .expect("standard variant id should exist");
    program.push_variant(vela_bytecode::LinkedVariant::new(
        variant_id, ty, debug_name,
    ))
}

fn linked_bytes_get_cache_program() -> (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    vela_def::MethodId,
) {
    let method_id = vela_stdlib::std_method_id("Bytes", "get").expect("Bytes::get method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("get");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 3);
    let receiver = code.push_constant(Constant::Bytes(vec![13, 21, 34]));
    let index = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(1)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: receiver,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(1),
            constant: index,
        },
    ));
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(2));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(2),
            receiver: Register(0),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn linked_string_contains_cache_program() -> (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    vela_def::MethodId,
) {
    let method_id =
        vela_stdlib::std_method_id("String", "contains").expect("String::contains method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("contains");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 3);
    let receiver = code.push_constant(Constant::String("daily_quest".into()));
    let needle = code.push_constant(Constant::String("quest".into()));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: receiver,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(1),
            constant: needle,
        },
    ));
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(2));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(2),
            receiver: Register(0),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: vec![vela_bytecode::CallArgument::Register(Register(1))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(2) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

fn run_linked_method_cache_program(
    program: &vela_bytecode::LinkedProgram,
    caches: &RecordingMethodCaches,
) -> VmResult<RuntimeValue> {
    let code = program
        .functions()
        .find(|(_, code)| program.debug_name(code.debug_name) == "main")
        .map(|(_, code)| code)
        .expect("linked method cache fixture should have main");
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let mut budget = ExecutionBudget::unbounded();
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
        Some(&mut heap_execution),
        Some(&mut budget),
    )
}

struct RecordingMethodCaches {
    entries: RefCell<Vec<Option<MethodInlineCacheEntry>>>,
    set_count: Cell<usize>,
}

impl RecordingMethodCaches {
    fn new(len: usize) -> Self {
        Self {
            entries: RefCell::new(vec![None; len]),
            set_count: Cell::new(0),
        }
    }

    fn entry(&self, site: CacheSiteId) -> Option<MethodInlineCacheEntry> {
        self.entries.borrow().get(site.index()).copied().flatten()
    }

    fn prime(&self, site: CacheSiteId, entry: MethodInlineCacheEntry) {
        self.entries.borrow_mut()[site.index()] = Some(entry);
    }

    fn set_count(&self) -> usize {
        self.set_count.get()
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
        self.set_count.set(self.set_count.get() + 1);
    }
}
