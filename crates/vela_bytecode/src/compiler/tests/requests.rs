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
