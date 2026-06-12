use super::*;
use crate::value::Value as RuntimeValue;
use std::cell::{Cell, RefCell};

pub(super) fn linked_standard_len_cache_program() -> (
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

pub(super) fn linked_string_no_arg_cache_program(
    method: &str,
    receiver: &str,
) -> (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    vela_def::MethodId,
) {
    let method_id = vela_stdlib::std_method_id("String", method).expect("String method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name(method);
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 2);
    let receiver = code.push_constant(Constant::String(receiver.to_owned()));
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

pub(super) fn linked_string_one_arg_cache_program(
    method: &str,
    receiver: &str,
    arg: &str,
) -> (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    vela_def::MethodId,
) {
    linked_string_one_constant_arg_cache_program(method, receiver, Constant::String(arg.to_owned()))
}

pub(super) fn linked_string_one_constant_arg_cache_program(
    method: &str,
    receiver: &str,
    arg: Constant,
) -> (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    vela_def::MethodId,
) {
    let method_id = vela_stdlib::std_method_id("String", method).expect("String method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name(method);
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 3);
    let receiver = code.push_constant(Constant::String(receiver.to_owned()));
    let arg = code.push_constant(arg);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: receiver,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(1),
            constant: arg,
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

pub(super) fn linked_map_get_or_cache_program() -> (
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

pub(super) fn linked_option_is_some_cache_program() -> (
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

pub(super) fn linked_result_unwrap_or_cache_program() -> (
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

pub(super) fn linked_bytes_get_cache_program() -> (
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

pub(super) fn linked_bytes_slice_cache_program() -> (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    vela_def::MethodId,
) {
    let method_id = vela_stdlib::std_method_id("Bytes", "slice").expect("Bytes::slice method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("slice");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 4);
    let receiver = code.push_constant(Constant::Bytes(vec![13, 21, 34, 55]));
    let start = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(1)));
    let end = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(3)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(0),
            constant: receiver,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(1),
            constant: start,
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst {
            dst: Register(2),
            constant: end,
        },
    ));
    let site = code.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(3));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::CallMethod {
            dst: Register(3),
            receiver: Register(0),
            dispatch,
            debug_name: method_name,
            cache_site: Some(site),
            args: vec![
                vela_bytecode::CallArgument::Register(Register(1)),
                vela_bytecode::CallArgument::Register(Register(2)),
            ],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::Return { src: Register(3) },
    ));
    let function = program.push_function(code);
    program.set_entry_point(main_name, function);
    (program, site, dispatch, method_id)
}

pub(super) fn linked_bytes_to_hex_cache_program() -> (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    vela_def::MethodId,
) {
    let method_id = vela_stdlib::std_method_id("Bytes", "to_hex").expect("Bytes::to_hex method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("to_hex");
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 2);
    let receiver = code.push_constant(Constant::Bytes(vec![13, 21, 34]));
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

pub(super) fn linked_string_contains_cache_program() -> (
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
    let mut heap = ScriptHeap::new();
    let mut heap_execution = HeapExecution::new(&mut heap);
    let result = run_linked_method_cache_with_heap(program, caches, &mut heap_execution)?;
    crate::heap_values::value_to_owned(&result, Some(&heap_execution))
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
    let code = program
        .functions()
        .find(|(_, code)| program.debug_name(code.debug_name) == "main")
        .map(|(_, code)| code)
        .expect("linked method cache fixture should have main");
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
        Some(heap_execution),
        Some(&mut budget),
    )
}

pub(super) struct RecordingMethodCaches {
    entries: RefCell<Vec<Option<MethodInlineCacheEntry>>>,
    set_count: Cell<usize>,
}

impl RecordingMethodCaches {
    pub(super) fn new(len: usize) -> Self {
        Self {
            entries: RefCell::new(vec![None; len]),
            set_count: Cell::new(0),
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
        self.set_count.set(self.set_count.get() + 1);
    }
}
