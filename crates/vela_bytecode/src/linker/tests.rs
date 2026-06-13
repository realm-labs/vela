use vela_common::{HostTypeId, SourceId};
use vela_def::FieldId;
use vela_host::target::HostTargetPlan;
use vela_registry::{DefinitionRegistry, FunctionDef, FunctionSignature};

use super::*;
use crate::compiler::compile_function_source_with_registry;
use crate::{CacheSiteKind, InstructionOffset, Register};

#[test]
fn linker_fails_on_unresolved_native_calls() {
    let native = FunctionId::new(100);
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: None,
            name: "missing".to_owned(),
            native,
            cache_site: None,
            args: Vec::new(),
        },
    ));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    let registry = DefinitionRegistry::new();

    let error = Linker::with_registry(&registry)
        .with_native_implementation(native)
        .link_program(&program)
        .expect_err("native definition should be required");

    assert!(matches!(
        error,
        LinkError::UnresolvedNative { name, id } if name == "missing" && id == native
    ));
}

#[test]
fn linker_fails_on_missing_native_implementation() {
    let path = DefPath::function("host", std::iter::empty::<&str>(), "award");
    let native = FunctionId::from_def_id(path.id());
    let mut registry = DefinitionRegistry::new();
    registry
        .register_function(FunctionDef::new(path, FunctionSignature::default()))
        .expect("native definition registration should succeed");
    let mut code = UnlinkedCodeObject::new("main", 1);
    let cache_site = code.push_cache_site(CacheSiteKind::NativeCall, InstructionOffset(0));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: None,
            name: "award".to_owned(),
            native,
            cache_site: Some(cache_site),
            args: Vec::new(),
        },
    ));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);

    let error = Linker::with_registry(&registry)
        .link_program(&program)
        .expect_err("native implementation should be required");

    assert!(matches!(
        error,
        LinkError::MissingNativeImplementation { name, id }
            if name == "award" && id == native
    ));
}

#[test]
fn linker_maps_native_functions_to_dense_handles() {
    let path = DefPath::function("host", std::iter::empty::<&str>(), "award");
    let native = FunctionId::from_def_id(path.id());
    let mut registry = DefinitionRegistry::new();
    registry
        .register_function(FunctionDef::new(path, FunctionSignature::default()))
        .expect("native definition registration should succeed");
    let mut code = UnlinkedCodeObject::new("main", 1);
    let native_cache_site = code.push_cache_site(CacheSiteKind::NativeCall, InstructionOffset(0));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: None,
            name: "award".to_owned(),
            native,
            cache_site: Some(native_cache_site),
            args: Vec::new(),
        },
    ));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);

    let linked = Linker::with_registry(&registry)
        .with_native_implementation(native)
        .link_program(&program)
        .expect("native should link");

    assert_eq!(linked.native_function_count(), 1);
    let main = linked
        .functions()
        .find(|(_, code)| linked.debug_name(code.debug_name) == "main")
        .map(|(_, code)| code)
        .expect("main function should link");
    assert!(matches!(
        main.instructions[0].kind,
        InstructionKind::CallNative {
            native: handle,
            cache_site: Some(site),
            ..
        } if handle.index() == 0 && site == native_cache_site
    ));
    let linked_native = linked
        .native_function(NativeHandle::new(0))
        .expect("native side table should contain handle 0");
    assert_eq!(linked_native.id, native);
    assert_eq!(linked.debug_name(linked_native.debug_name), "award");
}

#[test]
fn linker_preserves_i64_typed_instructions() {
    let mut code = UnlinkedCodeObject::new("main", 3);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::I64AddImm {
            dst: Register(1),
            lhs: Register(0),
            imm: 4,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::I64GtImm {
            dst: Register(2),
            lhs: Register(1),
            imm: 10,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);

    let linked = Linker::new()
        .link_program(&program)
        .expect("typed scalar instructions should link");
    let main = linked
        .functions()
        .find(|(_, code)| linked.debug_name(code.debug_name) == "main")
        .map(|(_, code)| code)
        .expect("main function should link");

    assert!(matches!(
        main.instructions[0].kind,
        InstructionKind::I64AddImm {
            dst: Register(1),
            lhs: Register(0),
            imm: 4
        }
    ));
    assert!(matches!(
        main.instructions[1].kind,
        InstructionKind::I64GtImm {
            dst: Register(2),
            lhs: Register(1),
            imm: 10
        }
    ));
}

#[test]
fn linker_maps_script_functions_and_methods_to_dense_handles() {
    let mut helper = UnlinkedCodeObject::new("helper", 1);
    helper.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(0),
    }));
    let mut main = UnlinkedCodeObject::new("main", 2);
    main.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallFunction {
            dst: Register(0),
            target: function_id_for_script_name("helper"),
            name: "helper".to_owned(),
            mode: crate::ScriptCallMode::Unchecked,
            args: Vec::new(),
        },
    ));
    let method = MethodId::new(200);
    main.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(1),
            receiver: Register(0),
            method: "score".to_owned(),
            method_id: method,
            args: Vec::new(),
        },
    ));
    let mut program = UnlinkedProgram::new();
    program.insert_function(helper);
    program.insert_function(main);
    program.insert_script_method("Player", "score", method, "helper");

    let linked = Linker::new()
        .link_program(&program)
        .expect("script calls should link");

    let main = linked
        .functions()
        .find(|(_, code)| linked.debug_name(code.debug_name) == "main")
        .map(|(_, code)| code)
        .expect("main function should link");
    let helper_handle = linked
        .functions()
        .find(|(_, code)| linked.debug_name(code.debug_name) == "helper")
        .map(|(handle, _)| handle)
        .expect("helper function should link");
    assert!(matches!(
        main.instructions[0].kind,
        InstructionKind::CallFunction {
            function,
            ..
        } if function == helper_handle
    ));
    assert!(matches!(
        main.instructions[1].kind,
        InstructionKind::CallMethod {
            dispatch,
            ..
        } if linked.method_dispatch(dispatch).is_some_and(|dispatch| {
            dispatch.kind == LinkedMethodDispatchKind::Script {
                method_id: method,
                function: helper_handle,
            }
        })
    ));
}

#[test]
fn linker_preserves_method_call_cache_site_operand() {
    let method = MethodId::new(200);
    let mut main = UnlinkedCodeObject::new("main", 2);
    let cache_site = main.push_cache_site(CacheSiteKind::MethodCall, InstructionOffset(0));
    main.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethodId {
            dst: Register(1),
            receiver: Register(0),
            method: "score".to_owned(),
            method_id: method,
            args: Vec::new(),
        },
    ));
    main.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(1),
    }));
    let mut program = UnlinkedProgram::new();
    program.insert_function(main);
    program.insert_script_method("Player", "score", method, "main");

    let linked = Linker::new()
        .link_program(&program)
        .expect("method call should link");
    let main = linked
        .functions()
        .find(|(_, code)| linked.debug_name(code.debug_name) == "main")
        .map(|(_, code)| code)
        .expect("main function should link");

    assert!(matches!(
        main.instructions[0].kind,
        InstructionKind::CallMethod {
            cache_site: Some(site),
            ..
        } if site == cache_site
    ));
}

#[test]
fn linker_links_unknown_receiver_source_dynamic_methods() {
    assert_linked_dynamic_method_source("starts_with", r#"return value.starts_with("q");"#);
    assert_linked_dynamic_method_source("trim", "return value.trim();");
}

fn assert_linked_dynamic_method_source(method: &str, body: &str) {
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let code = compile_function_source_with_registry(
        SourceId::new(1),
        &format!("fn f(value) {{ {body} }}"),
        "f",
        registry.compile_view(),
    )
    .expect("dynamic method source should compile");
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);
    let linked = Linker::new()
        .link_program(&program)
        .expect("dynamic method source should link");
    let function = linked
        .entry_point_by_name("f")
        .and_then(|handle| linked.function(handle))
        .expect("linked function");
    assert!(function.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::CallDynamicMethod {
            method_name,
            cache_site: Some(_),
            ..
        } if linked.debug_name(*method_name) == method
    )));
}

#[test]
fn linker_rejects_script_call_with_matching_name_and_wrong_id() {
    let mut helper = UnlinkedCodeObject::new("helper", 1);
    helper.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(0),
    }));
    let wrong_id = FunctionId::new(999);
    let mut main = UnlinkedCodeObject::new("main", 1);
    main.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallFunction {
            dst: Register(0),
            target: wrong_id,
            name: "helper".to_owned(),
            mode: crate::ScriptCallMode::Checked,
            args: Vec::new(),
        },
    ));
    let mut program = UnlinkedProgram::new();
    program.insert_function(helper);
    program.insert_function(main);

    let error = Linker::new()
        .link_program(&program)
        .expect_err("script calls should resolve by id, not matching debug name");

    assert!(matches!(
        error,
        LinkError::MissingScriptFunction { name, id } if name == "helper" && id == wrong_id
    ));
}

#[test]
fn linker_maps_globals_map_keys_and_field_slots_without_instruction_names() {
    let mut code = UnlinkedCodeObject::new("main", 4);
    code.push_constant(Constant::Scalar(vela_common::ScalarValue::I64(1)));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::LoadGlobal {
            dst: Register(0),
            global: "main::state".to_owned(),
            slot: None,
            cache_site: None,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::MakeMap {
        dst: Register(1),
        entries: vec![("score".to_owned(), Register(0))],
    }));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::MakeRecord {
            dst: Register(2),
            type_name: "Player".to_owned(),
            fields: vec![
                ("score".to_owned(), Register(0)),
                ("level".to_owned(), Register(1)),
            ],
        },
    ));
    let read_site = code.push_cache_site(CacheSiteKind::RecordFieldRead, InstructionOffset(3));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::GetRecordSlot {
            dst: Register(3),
            record: Register(2),
            field: "score".to_owned(),
            slot: 1,
        },
    ));
    let write_site = code.push_cache_site(CacheSiteKind::RecordFieldWrite, InstructionOffset(4));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::SetRecordSlot {
            record: Register(2),
            field: "level".to_owned(),
            slot: 0,
            src: Register(0),
        },
    ));
    let mut program = UnlinkedProgram::new();
    program.set_global_layout(["main::state".to_owned()]);
    program.insert_function(code);

    let linked = Linker::new()
        .link_program(&program)
        .expect("global and field slots should link");
    let main = linked
        .functions()
        .find(|(_, code)| linked.debug_name(code.debug_name) == "main")
        .map(|(_, code)| code)
        .expect("main function should link");

    assert!(matches!(
        main.instructions[0].kind,
        InstructionKind::LoadGlobal { slot, .. } if slot == vela_common::GlobalSlot::new(0)
    ));
    assert!(matches!(
        &main.instructions[1].kind,
        InstructionKind::MakeMap { entries, .. }
            if matches!(main.constants[entries[0].0.0], Constant::String(ref key) if key == "score")
    ));
    assert!(matches!(
        &main.instructions[2].kind,
        InstructionKind::MakeRecord { fields, .. }
            if fields.iter().map(|(slot, debug_name, register)| (*slot, linked.debug_name(*debug_name), *register)).collect::<Vec<_>>()
                == vec![
                    (FieldSlot::new(1), "score", Register(0)),
                    (FieldSlot::new(0), "level", Register(1)),
                ]
    ));
    assert!(matches!(
        main.instructions[3].kind,
        InstructionKind::GetRecordSlot { field, debug_name, cache_site: Some(site), .. }
            if field == FieldSlot::new(1) && linked.debug_name(debug_name) == "score" && site == read_site
    ));
    assert!(matches!(
        main.instructions[4].kind,
        InstructionKind::SetRecordSlot { field, debug_name, cache_site: Some(site), .. }
            if field == FieldSlot::new(0) && linked.debug_name(debug_name) == "level" && site == write_site
    ));
}

#[test]
fn linker_links_dynamic_method_and_rejects_record_field_fallbacks() {
    let mut method_code = UnlinkedCodeObject::new("method", 2);
    method_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallDynamicMethod {
            dst: Register(0),
            receiver: Register(1),
            method: "score".to_owned(),
            args: Vec::new(),
        },
    ));
    let mut method_program = UnlinkedProgram::new();
    method_program.insert_function(method_code);

    let linked = Linker::new()
        .link_program(&method_program)
        .expect("dynamic method dispatch should link");
    let method = linked
        .entry_point_by_name("method")
        .and_then(|handle| linked.function(handle))
        .expect("linked method function");
    assert!(matches!(
        &method.instructions[0].kind,
        InstructionKind::CallDynamicMethod { method_name, args, .. }
            if linked.debug_name(*method_name) == "score" && args.is_empty()
    ));

    let mut field_code = UnlinkedCodeObject::new("field", 2);
    field_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::GetRecordField {
            dst: Register(0),
            record: Register(1),
            field: "score".to_owned(),
        },
    ));
    let mut field_program = UnlinkedProgram::new();
    field_program.insert_function(field_code);

    let error = Linker::new()
        .link_program(&field_program)
        .expect_err("name-only record field dispatch should not link");
    assert!(matches!(error, LinkError::UnresolvedRecordField { .. }));
}

#[test]
fn linker_remaps_host_target_plans_and_host_methods_to_linked_handles() {
    let method = HostMethodId::new(13);
    let plan = HostTargetPlan::new(HostTypeId::new(7)).field(FieldId::new(11));
    let mut code = UnlinkedCodeObject::new("main", 3).with_params(vec!["player".to_owned()]);
    code.host_targets.push(plan.clone());
    code.host_targets.push(plan.clone());
    let read_site = code.push_cache_site(CacheSiteKind::HostPathRead, InstructionOffset(0));
    let call_site = code.push_cache_site(CacheSiteKind::HostPathCall, InstructionOffset(1));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostRead {
            dst: Register(1),
            root: Register(0),
            target: HostTargetPlanId::new(1),
            dynamic_args: Vec::new(),
            cache_site: read_site,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::HostCall {
            dst: Some(Register(2)),
            root: Register(0),
            target: HostTargetPlanId::new(1),
            dynamic_args: Vec::new(),
            method,
            args: vec![Register(1)],
            cache_site: call_site,
        },
    ));
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);

    let linked = Linker::new()
        .link_program(&program)
        .expect("host targets should link");
    let main = linked
        .functions()
        .find(|(_, code)| linked.debug_name(code.debug_name) == "main")
        .map(|(_, code)| code)
        .expect("main function should link");

    assert_eq!(main.host_targets, vec![plan]);
    assert!(matches!(
        main.instructions[0].kind,
        InstructionKind::HostRead {
            target,
            ..
        } if target == HostTargetPlanId::new(0)
    ));
    assert!(matches!(
        main.instructions[1].kind,
        InstructionKind::HostCall {
            target,
            method: dispatch,
            ..
        } if target == HostTargetPlanId::new(0)
            && linked.method_dispatch(dispatch).is_some_and(|dispatch| {
                dispatch.kind == LinkedMethodDispatchKind::Host { method_id: method }
            })
    ));
}
