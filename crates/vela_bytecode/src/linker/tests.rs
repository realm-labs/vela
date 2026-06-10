use vela_registry::{DefinitionRegistry, FunctionDef, FunctionSignature};

use super::*;
use crate::Register;

#[test]
fn linker_fails_on_unresolved_native_calls() {
    let native = FunctionId::new(100);
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: None,
            name: "missing".to_owned(),
            native,
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
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: None,
            name: "award".to_owned(),
            native,
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
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallNative {
            dst: None,
            name: "award".to_owned(),
            native,
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
            ..
        } if handle.index() == 0
    ));
    let linked_native = linked
        .native_function(NativeHandle::new(0))
        .expect("native side table should contain handle 0");
    assert_eq!(linked_native.id, native);
    assert_eq!(linked.debug_name(linked_native.debug_name), "award");
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
    code.push_constant(Constant::Int(1));
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
    code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::GetRecordSlot {
            dst: Register(3),
            record: Register(2),
            field: "score".to_owned(),
            slot: 1,
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
        InstructionKind::GetRecordSlot { field, debug_name, .. }
            if field == FieldSlot::new(1) && linked.debug_name(debug_name) == "score"
    ));
}

#[test]
fn linker_rejects_name_only_method_and_record_field_fallbacks() {
    let mut method_code = UnlinkedCodeObject::new("method", 2);
    method_code.push_instruction(UnlinkedInstruction::new(
        UnlinkedInstructionKind::CallMethod {
            dst: Register(0),
            receiver: Register(1),
            method: "score".to_owned(),
            args: Vec::new(),
        },
    ));
    let mut method_program = UnlinkedProgram::new();
    method_program.insert_function(method_code);

    let error = Linker::new()
        .link_program(&method_program)
        .expect_err("name-only method dispatch should not link");
    assert!(matches!(error, LinkError::UnresolvedMethodName { .. }));

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
