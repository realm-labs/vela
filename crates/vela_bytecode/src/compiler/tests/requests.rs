use super::*;

#[test]
fn compiler_entry_points_return_unlinked_bytecode() {
    let program: super::CompiledProgram = compile_test_program(
        SourceId::new(1),
        r#"
fn main() {
    return 42;
}
"#,
    )
    .expect("program should compile");
    assert!(program.function("main").is_some());

    let code: UnlinkedCodeObject = compile_test_function(
        SourceId::new(2),
        r#"
fn main() {
    return 7;
}
"#,
        "main",
    )
    .expect("function should compile");
    assert_eq!(code.name, "main");
}

#[test]
fn compiler_and_linker_preserve_function_asyncness() {
    let program = compile_test_program(
        SourceId::new(4),
        "async fn pending() { return 7; } fn ready() { return 8; }",
    )
    .expect("async metadata source should compile");

    assert_eq!(
        program
            .function("pending")
            .expect("pending bytecode")
            .asyncness,
        vela_common::CallableAsyncness::Async
    );
    assert_eq!(
        program.function("ready").expect("ready bytecode").asyncness,
        vela_common::CallableAsyncness::Sync
    );

    let artifact = crate::Linker::new()
        .link_compiled_program(program)
        .expect("async metadata program should link");
    let linked = artifact.program();
    let pending = linked
        .entry_point_by_name("pending")
        .and_then(|handle| linked.function(handle))
        .expect("linked pending entry");
    let ready = linked
        .entry_point_by_name("ready")
        .and_then(|handle| linked.function(handle))
        .expect("linked ready entry");
    assert_eq!(pending.asyncness, vela_common::CallableAsyncness::Async);
    assert_eq!(ready.asyncness, vela_common::CallableAsyncness::Sync);
}

#[test]
fn compiler_and_linker_emit_sealed_scoped_task_instruction() {
    let program = compile_test_program(
        SourceId::new(5),
        r#"
async fn repair(value: i64) -> i64 { return value + 1; }
fn main() {
    task::spawn_scoped(repair(41));
}
"#,
    )
    .expect("scoped task source should compile");
    let main = program.function("main").expect("main bytecode");
    assert!(matches!(
        &main.instructions[1].kind,
        UnlinkedInstructionKind::Task(task)
            if task.worker_name == "repair"
                && task.args.len() == 1
                && task.continuation.is_none()
    ));

    let artifact = crate::Linker::new()
        .link_compiled_program(program)
        .expect("scoped task program should link");
    assert!(
        artifact
            .required_features()
            .contains(crate::ArtifactFeatureSet::host_scoped_tasks())
    );
    let target = artifact.task_targets().first().expect("sealed task target");
    assert_eq!(target.worker_debug_name, "repair");
    assert_eq!(target.worker_signature.parameter_detachability.len(), 1);
    assert!(target.continuation.is_none());
}

#[test]
fn graph_requests_compile_program_and_stable_function_roots() {
    let built = vela_hir::source_ingestion::build_single_source(
        SourceId::new(3),
        "fn helper() { return 2; } fn main() { return helper(); }",
    )
    .expect("HIR source set");
    let function = built
        .function(&anonymous_module(ModulePath::root()), "main")
        .expect("stable main declaration");
    let options = CompilerOptions::default();
    let program = compile_program(ProgramCompilationRequest {
        sources: &built,
        options: &options,
        registry: None,
    })
    .expect("graph program request");
    let code = compile_function(FunctionCompilationRequest {
        function,
        options: &options,
        registry: None,
    })
    .expect("stable function request");

    assert!(program.function("main").is_some());
    assert_eq!(code.name, "main");
}

#[test]
fn compilation_requests_reject_invalid_scope_and_function_roots() {
    let options = CompilerOptions::default();
    let empty = vela_hir::source_ingestion::build_module_source_set(&[]).expect("empty source set");
    let error = compile_program(ProgramCompilationRequest {
        sources: &empty,
        options: &options,
        registry: None,
    })
    .expect_err("empty module graph must be rejected");
    assert_eq!(
        error.kind,
        CompileErrorKind::InvalidCompilationRequest(
            error::CompilationRequestError::EmptyModuleGraph
        )
    );

    let built = vela_hir::source_ingestion::build_single_source(
        SourceId::new(31),
        "const VALUE = 1; fn first() { return VALUE; }",
    )
    .expect("single source set");
    assert!(
        built
            .function(&anonymous_module(ModulePath::root()), "VALUE")
            .is_none()
    );
    assert!(
        built
            .function(&anonymous_module(ModulePath::root()), "missing")
            .is_none()
    );
}

#[test]
fn function_selection_is_bound_to_its_source_set_even_when_hir_ids_collide() {
    let alpha = vela_hir::source_ingestion::build_single_source(
        SourceId::new(33),
        "fn alpha() { return 1; }",
    )
    .expect("alpha source set");
    let beta = vela_hir::source_ingestion::build_single_source(
        SourceId::new(34),
        "fn beta() { return 2; }",
    )
    .expect("beta source set");
    let alpha = alpha
        .function(&anonymous_module(ModulePath::root()), "alpha")
        .expect("alpha function");
    let beta = beta
        .function(&anonymous_module(ModulePath::root()), "beta")
        .expect("beta function");
    assert_eq!(alpha.declaration(), beta.declaration());

    let code = compile_function(FunctionCompilationRequest {
        function: beta,
        options: &CompilerOptions::default(),
        registry: None,
    })
    .expect("bound beta function compiles");
    assert_eq!(code.name, "beta");
}
