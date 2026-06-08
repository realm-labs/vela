use super::*;
use crate::owned_value::OwnedValue;

#[test]
fn compiled_source_reflects_modules_functions_and_exports() {
    let program = compile_program_source(
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
    if module.name == "game::reward"
        && reflect::name(module) == "game::reward"
        && reflect::kind(module) == "module"
        && reflect::source_span(module) != null
        && reflect::has_module("game::reward")
        && !reflect::has_module("game::missing")
        && reflect::has_function("game::reward::grant")
        && !reflect::has_function("game::reward::missing")
        && modules.len() == 1
        && modules[0].name == "game::reward"
        && exports.len() == 1
        && module_exports.len() == 1
        && listed_exports[0] == "game::reward::grant"
        && functions.len() == 1
        && functions[0].name == "game::reward::grant"
        && functions[0].id == function.id
        && reflect::name(function) == "game::reward::grant"
        && reflect::id(function) == function.id
        && reflect::kind(function) == "function"
        && function.id > 0
        && reflect::docs(function) == "Grant reward."
        && reflect::origin(function) == "script"
        && reflect::origin(module) == "script"
        && reflect::attr(function, "event") == "reward"
        && reflect::source_span(function).source == 1
        && reflect::get(function, "return") == "bool" {
        return function.params.len();
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
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(OwnedValue::Int(2))
    );
}

#[test]
fn compiled_source_reflect_module_reports_unknown_module_candidates() {
    let program = compile_program_source(
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

    let error = vm
        .run_program_with_host(&program, "main", &[], &mut host)
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
    let program = compile_program_source(
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

    let error = vm
        .run_program_with_host(&program, "main", &[], &mut host)
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
    let program = compile_program_source(
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

    let error = vm
        .run_program_with_host(&program, "main", &[], &mut host)
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
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct ReflectFunction { name: string }

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

    let error = vm
        .run_program_with_host(&program, "main", &[], &mut host)
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
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    let module = reflect::module("game::reward");
    let modules = reflect::modules();
    let exports = reflect::exports("game::reward");
    let module_exports = reflect::exports(module);
    let functions = reflect::functions();
    if reflect::has_module("game::reward")
        && !reflect::has_module("game::missing")
        && reflect::has_function("game::reward::grant")
        && !reflect::has_function("game::reward::hidden")
        && !reflect::has_function("game::reward::private")
        && !reflect::has_function("game::reward::admin") {
        return module.exports.len() * 100
            + exports.len() * 10
            + module_exports.len() * 10000
            + functions.len()
            + modules[0].exports.len() * 1000;
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
        vm.run_program_with_host(&program, "main", &[], &mut host),
        Ok(OwnedValue::Int(11111))
    );
}
