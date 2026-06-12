use super::linked_standard_method_cache_support::*;
use super::*;

type LinkedOptionResultCacheFixture = (
    vela_bytecode::LinkedProgram,
    CacheSiteId,
    vela_bytecode::MethodDispatchHandle,
    MethodId,
);

#[derive(Clone, Copy)]
enum StandardEnum {
    Option,
    Result,
}

impl StandardEnum {
    fn name(self) -> &'static str {
        match self {
            Self::Option => "Option",
            Self::Result => "Result",
        }
    }
}

#[derive(Clone, Copy)]
enum StandardVariant {
    Some,
    Ok,
    Err,
}

impl StandardVariant {
    fn name(self) -> &'static str {
        match self {
            Self::Some => "Some",
            Self::Ok => "Ok",
            Self::Err => "Err",
        }
    }
}

#[test]
fn linked_standard_value_method_caches_option_ok_or_target() {
    assert_option_result_owned_cache(
        linked_option_ok_or_cache_program(),
        StandardMethodReceiver::Option,
        StandardMethodInlineCacheTarget::OkOr,
        owned_result_ok(OwnedValue::i64(4)),
    );
}

#[test]
fn linked_standard_value_method_caches_option_flatten_target() {
    assert_option_result_owned_cache(
        linked_nested_enum_cache_program(
            StandardEnum::Option,
            StandardVariant::Some,
            StandardEnum::Option,
            StandardVariant::Some,
            "flatten",
            6,
        ),
        StandardMethodReceiver::Option,
        StandardMethodInlineCacheTarget::Flatten,
        owned_option_some(OwnedValue::i64(6)),
    );
}

#[test]
fn linked_standard_value_method_caches_result_to_option_target() {
    assert_option_result_owned_cache(
        linked_enum_no_arg_cache_program(StandardEnum::Result, StandardVariant::Ok, "to_option", 8),
        StandardMethodReceiver::Result,
        StandardMethodInlineCacheTarget::ToOption,
        owned_option_some(OwnedValue::i64(8)),
    );
}

#[test]
fn linked_standard_value_method_caches_result_to_error_option_target() {
    assert_option_result_owned_cache(
        linked_enum_no_arg_cache_program(
            StandardEnum::Result,
            StandardVariant::Err,
            "to_error_option",
            404,
        ),
        StandardMethodReceiver::Result,
        StandardMethodInlineCacheTarget::ToErrorOption,
        owned_option_some(OwnedValue::i64(404)),
    );
}

#[test]
fn linked_standard_value_method_caches_result_flatten_target() {
    assert_option_result_owned_cache(
        linked_nested_enum_cache_program(
            StandardEnum::Result,
            StandardVariant::Ok,
            StandardEnum::Result,
            StandardVariant::Ok,
            "flatten",
            10,
        ),
        StandardMethodReceiver::Result,
        StandardMethodInlineCacheTarget::Flatten,
        owned_result_ok(OwnedValue::i64(10)),
    );
}

fn assert_option_result_owned_cache(
    fixture: LinkedOptionResultCacheFixture,
    receiver: StandardMethodReceiver,
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
    assert_option_result_cache_entry(&caches, site, dispatch, method_id, receiver, target);
    assert_eq!(caches.set_count(), 2);

    assert_eq!(
        run_linked_method_cache_owned_program(&program, &caches),
        expected
    );
    assert_eq!(caches.set_count(), 2);
}

fn assert_option_result_cache_entry(
    caches: &RecordingMethodCaches,
    site: CacheSiteId,
    dispatch: vela_bytecode::MethodDispatchHandle,
    method_id: MethodId,
    receiver: StandardMethodReceiver,
    target: StandardMethodInlineCacheTarget,
) {
    let entry = caches
        .entry(site)
        .expect("standard Option/Result cache should populate");
    assert_eq!(entry.dispatch, dispatch);
    let MethodInlineCacheTarget::Value {
        method_id: cached_method,
        standard_method: Some(standard_method),
    } = entry.target
    else {
        panic!("standard Option/Result cache should store value target");
    };
    assert_eq!(cached_method, method_id);
    assert_eq!(standard_method.receiver, receiver);
    assert_eq!(standard_method.target, target);
}

fn linked_option_ok_or_cache_program() -> LinkedOptionResultCacheFixture {
    let method_id = vela_stdlib::std_method_id("Option", "ok_or").expect("Option::ok_or method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name("ok_or");
    let field_name = program.intern_debug_name("0");
    let option_type = push_standard_type(&mut program, StandardEnum::Option);
    let some_variant = push_standard_variant(
        &mut program,
        option_type,
        StandardEnum::Option,
        StandardVariant::Some,
    );
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 4);
    load_i64(&mut code, Register(0), 4);
    load_i64(&mut code, Register(1), 99);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeEnum {
            dst: Register(2),
            enum_ty: option_type,
            variant: some_variant,
            fields: vec![(vela_bytecode::FieldSlot::new(0), field_name, Register(0))],
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

fn linked_enum_no_arg_cache_program(
    enum_kind: StandardEnum,
    variant_kind: StandardVariant,
    method: &str,
    payload: i64,
) -> LinkedOptionResultCacheFixture {
    let method_id = vela_stdlib::std_method_id(enum_kind.name(), method)
        .expect("standard Option/Result method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name(method);
    let field_name = program.intern_debug_name("0");
    let enum_type = push_standard_type(&mut program, enum_kind);
    let variant = push_standard_variant(&mut program, enum_type, enum_kind, variant_kind);
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 3);
    load_i64(&mut code, Register(0), payload);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeEnum {
            dst: Register(1),
            enum_ty: enum_type,
            variant,
            fields: vec![(vela_bytecode::FieldSlot::new(0), field_name, Register(0))],
        },
    ));
    let site = code.push_cache_site(
        CacheSiteKind::MethodCall,
        InstructionOffset(code.instructions.len()),
    );
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

fn linked_nested_enum_cache_program(
    outer_enum: StandardEnum,
    outer_variant: StandardVariant,
    inner_enum: StandardEnum,
    inner_variant: StandardVariant,
    method: &str,
    payload: i64,
) -> LinkedOptionResultCacheFixture {
    let method_id = vela_stdlib::std_method_id(outer_enum.name(), method)
        .expect("standard Option/Result method id");
    let mut program = vela_bytecode::LinkedProgram::new();
    let main_name = program.intern_debug_name("main");
    let method_name = program.intern_debug_name(method);
    let field_name = program.intern_debug_name("0");
    let inner_type = push_standard_type(&mut program, inner_enum);
    let inner_variant = push_standard_variant(&mut program, inner_type, inner_enum, inner_variant);
    let outer_type = push_standard_type(&mut program, outer_enum);
    let outer_variant = push_standard_variant(&mut program, outer_type, outer_enum, outer_variant);
    let dispatch = program.push_method_dispatch(vela_bytecode::LinkedMethodDispatch::new(
        method_name,
        vela_bytecode::LinkedMethodDispatchKind::Value { method_id },
    ));

    let mut code = vela_bytecode::LinkedCodeObject::new(main_name, 4);
    load_i64(&mut code, Register(0), payload);
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeEnum {
            dst: Register(1),
            enum_ty: inner_type,
            variant: inner_variant,
            fields: vec![(vela_bytecode::FieldSlot::new(0), field_name, Register(0))],
        },
    ));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::MakeEnum {
            dst: Register(2),
            enum_ty: outer_type,
            variant: outer_variant,
            fields: vec![(vela_bytecode::FieldSlot::new(0), field_name, Register(1))],
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

fn push_standard_type(
    program: &mut vela_bytecode::LinkedProgram,
    enum_kind: StandardEnum,
) -> vela_bytecode::TypeHandle {
    let debug_name = program.intern_debug_name(enum_kind.name());
    let type_id =
        vela_stdlib::std_type_id(enum_kind.name()).expect("standard type id should exist");
    program.push_type(vela_bytecode::LinkedType::new(type_id, debug_name))
}

fn push_standard_variant(
    program: &mut vela_bytecode::LinkedProgram,
    ty: vela_bytecode::TypeHandle,
    enum_kind: StandardEnum,
    variant_kind: StandardVariant,
) -> vela_bytecode::VariantHandle {
    let debug_name =
        program.intern_debug_name(format!("{}::{}", enum_kind.name(), variant_kind.name()));
    let variant_id = vela_stdlib::std_variant_id(enum_kind.name(), variant_kind.name())
        .expect("standard variant id should exist");
    program.push_variant(vela_bytecode::LinkedVariant::new(
        variant_id, ty, debug_name,
    ))
}

fn load_i64(code: &mut vela_bytecode::LinkedCodeObject, dst: Register, value: i64) {
    let constant = code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(value)));
    code.push_instruction(vela_bytecode::linked::Instruction::new(
        vela_bytecode::linked::InstructionKind::LoadConst { dst, constant },
    ));
}

fn owned_result_ok(value: OwnedValue) -> OwnedValue {
    owned_result("Ok", value)
}

fn owned_result(variant: &str, value: OwnedValue) -> OwnedValue {
    OwnedValue::Enum {
        enum_name: "Result".to_owned(),
        variant: variant.to_owned(),
        fields: ScriptFields::single(&format!("Result::{variant}"), "0", value),
    }
}
