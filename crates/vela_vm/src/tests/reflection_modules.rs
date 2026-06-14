use super::*;
use crate::owned_value::OwnedValue;

fn exec_reflection_module_program(
    vm: &Vm,
    program: &UnlinkedProgram,
    host: &mut HostExecution<'_>,
) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_host_budget(vm, program, "main", &[], host, &mut budget)
}

fn compile_reflection_module_source(
    source: SourceId,
    text: &str,
) -> vela_bytecode::compiler::error::CompileResult<UnlinkedProgram> {
    compile_standard_program_source_with_native_functions(
        source,
        text,
        &[
            "reflect::attr",
            "reflect::call",
            "reflect::docs",
            "reflect::exports",
            "reflect::function",
            "reflect::functions",
            "reflect::get",
            "reflect::has_function",
            "reflect::has_module",
            "reflect::id",
            "reflect::kind",
            "reflect::module",
            "reflect::modules",
            "reflect::name",
            "reflect::origin",
            "reflect::source_span",
        ],
    )
}

#[test]
fn compiled_source_reflects_modules_functions_and_exports() {
    let program = compile_reflection_module_source(
        SourceId::new(1),
        r#"
fn main() {
    let module = reflect::module("game::reward");
    let modules = reflect::modules();
    let exports = reflect::exports("game::reward");
    let module_exports = reflect::exports(module);
    let listed_exports = reflect::exports(modules[0]);
    let function = reflect::function("game::reward::grant");
    let functions = reflect::functions();
    let function_params = reflect::get(function, "params");
    if reflect::get(module, "name") == "game::reward"
        && reflect::name(module) == "game::reward"
        && reflect::kind(module) == "module"
        && reflect::source_span(module) != null
        && reflect::has_module("game::reward")
        && !reflect::has_module("game::missing")
        && reflect::has_function("game::reward::grant")
        && !reflect::has_function("game::reward::missing")
        && reflect::get(modules[0], "name") == "game::reward"
        && exports[0] == "game::reward::grant"
        && module_exports[0] == "game::reward::grant"
        && listed_exports[0] == "game::reward::grant"
        && reflect::get(functions[0], "name") == "game::reward::grant"
        && reflect::get(functions[0], "id") == reflect::get(function, "id")
        && reflect::get(function_params[0], "name") == "player"
        && reflect::get(function_params[1], "name") == "amount"
        && reflect::name(function) == "game::reward::grant"
        && reflect::id(function) == reflect::get(function, "id")
        && reflect::kind(function) == "function"
        && reflect::get(function, "id") > 0
        && reflect::docs(function) == "Grant reward."
        && reflect::origin(function) == "script"
        && reflect::origin(module) == "script"
        && reflect::attr(function, "event") == "reward"
        && reflect::get(reflect::source_span(function), "source") == 1
        && reflect::get(function, "return") == "bool" {
        return 2;
    }
    return 0;
}
"#,
    )
    .expect("compile module reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_module_reflection_registry()));
    vm.register_standard_natives();
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        exec_reflection_module_program(&vm, &program, &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn compiled_source_reflect_module_reports_unknown_module_candidates() {
    let program = compile_reflection_module_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect::module("game::rewards");
}
"#,
    )
    .expect("compile unknown module reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_module_reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let error = exec_reflection_module_program(&vm, &program, &mut host)
        .expect_err("unknown module should report candidates");

    assert!(matches!(
        error.kind(),
        VmErrorKind::Reflect(ReflectErrorKind::UnknownModule {
            ref module,
            ref candidates,
            ref related,
        }) if module == "game::rewards"
            && candidates.len() == 1
            && candidates[0] == "game::reward"
            && related
                .iter()
                .any(|candidate| candidate.name == "game::reward")
    ));
}

#[test]
fn compiled_source_reflect_function_reports_unknown_function_candidates() {
    let program = compile_reflection_module_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect::function("game::reward::grnat");
}
"#,
    )
    .expect("compile unknown function reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(script_module_reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let error = exec_reflection_module_program(&vm, &program, &mut host)
        .expect_err("unknown function should report candidates");

    assert!(matches!(
        error.kind(),
        VmErrorKind::Reflect(ReflectErrorKind::UnknownFunction {
            ref function,
            ref candidates,
            ref related,
        }) if function == "game::reward::grnat"
            && candidates.len() == 1
            && candidates[0] == "game::reward::grant"
            && related
                .iter()
                .any(|candidate| candidate.name == "game::reward::grant")
    ));
}

#[test]
fn compiled_source_reflect_function_candidates_respect_policy() {
    let program = compile_reflection_module_source(
        SourceId::new(1),
        r#"
fn main() {
    return reflect::function("game::reward::hiddne");
}
"#,
    )
    .expect("compile policy unknown function reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(policy_module_reflection_registry()),
        reflect::permissions::ReflectPolicy::read_only(),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let error = exec_reflection_module_program(&vm, &program, &mut host)
        .expect_err("unknown function should report policy-visible candidates");

    assert_eq!(
        error.kind(),
        VmErrorKind::Reflect(ReflectErrorKind::UnknownFunction {
            function: "game::reward::hiddne".to_owned(),
            candidates: vec!["game::reward::grant".to_owned()],
            related: vec![ReflectCandidate::new("game::reward::grant", None)],
        })
    );
}

#[test]
fn compiled_source_reflect_call_function_candidates_respect_policy() {
    let program = compile_reflection_module_source(
        SourceId::new(1),
        r#"
struct ReflectFunction { name: String }

fn main() {
    let function = ReflectFunction { name: "game::reward::grant_visibel" };
    return reflect::call(function);
}
"#,
    )
    .expect("compile policy unknown function call reflection source");
    let mut registry = TypeRegistry::new();
    registry.register_function(
        FunctionDesc::new(FunctionId::new(1), "game::reward::grant")
            .access(FunctionAccess::new().reflect_callable(true)),
    );
    registry.register_function(FunctionDesc::new(
        FunctionId::new(2),
        "game::reward::grant_visible",
    ));
    registry.register_function(
        FunctionDesc::new(FunctionId::new(3), "game::reward::grant_hidden").access(
            FunctionAccess::new()
                .reflect_visible(false)
                .reflect_callable(true),
        ),
    );
    registry.register_function(
        FunctionDesc::new(FunctionId::new(4), "game::reward::grant_write").access(
            FunctionAccess::new()
                .reflect_callable(true)
                .require_permission("game::write"),
        ),
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives_with_policy(
        Arc::new(registry),
        reflect::permissions::ReflectPolicy::new(
            reflect::permissions::ReflectPermissionSet::new()
                .with(reflect::permissions::ReflectPermission::CallMethods),
        ),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    let error = exec_reflection_module_program(&vm, &program, &mut host)
        .expect_err("unknown function call should report callable candidates only");

    assert_eq!(
        error.kind(),
        VmErrorKind::Reflect(ReflectErrorKind::UnknownFunction {
            function: "game::reward::grant_visibel".to_owned(),
            candidates: vec!["game::reward::grant".to_owned()],
            related: vec![ReflectCandidate::new("game::reward::grant", None)],
        })
    );
}

#[test]
fn compiled_source_reflect_exports_respect_function_policy() {
    let program = compile_reflection_module_source(
        SourceId::new(1),
        r#"
fn main() {
    let module = reflect::module("game::reward");
    let modules = reflect::modules();
    let exports = reflect::exports("game::reward");
    let module_exports = reflect::exports(module);
    let functions = reflect::functions();
    let module_export_names = reflect::get(module, "exports");
    let listed_export_names = reflect::get(modules[0], "exports");
    if reflect::has_module("game::reward")
        && !reflect::has_module("game::missing")
        && reflect::has_function("game::reward::grant")
        && !reflect::has_function("game::reward::hidden")
        && !reflect::has_function("game::reward::private")
        && !reflect::has_function("game::reward::admin") {
        if module_export_names[0] == "game::reward::grant"
            && exports[0] == "game::reward::grant"
            && module_exports[0] == "game::reward::grant"
            && reflect::get(functions[0], "name") == "game::reward::grant"
            && listed_export_names[0] == "game::reward::grant" {
            return 11111;
        }
    }
    return 0;
}
"#,
    )
    .expect("compile policy exports reflection source");
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_standard_natives();
    vm.register_reflection_natives_with_policy(
        Arc::new(policy_module_reflection_registry()),
        reflect::permissions::ReflectPolicy::read_only(),
    );
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        exec_reflection_module_program(&vm, &program, &mut host),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(11111)))
    );
}
