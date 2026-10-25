use vela_common::SourceId;

#[test]
fn removed_task_admission_never_reaches_compile_targets() {
    for source in [
        "fn worker() {} fn main() { task::spawn_scoped(worker()); }",
        "fn worker() {} fn done(value) {} fn main() { task::spawn_scoped_then(worker(), done); }",
        "use task::spawn_scoped as spawn; fn main() { spawn(worker()); }",
        "use task as jobs; fn main() { jobs::spawn_scoped_then(worker(), done); }",
    ] {
        let error = crate::compiler::compile_test_program(SourceId::new(1), source)
            .expect_err("removed admission cannot produce compiler targets or bytecode");
        assert!(
            matches!(error.kind,
                crate::compiler::CompileErrorKind::InvalidHirGraph(ref diagnostics)
                    if diagnostics.iter().any(|diagnostic| diagnostic.code.as_deref() == Some("syntax::removed_task"))
            ),
            "{source}: {error:?}"
        );
    }
}
