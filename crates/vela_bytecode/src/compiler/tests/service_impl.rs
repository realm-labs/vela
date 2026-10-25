use super::*;

#[test]
fn compiler_rejects_removed_service_and_provider_declarations() {
    for source in [
        "#[service_impl(app::Inventory)] impl Patch { fn grant(value) { return value; } }",
        "#[service_impl] impl Patch { fn grant(value) { return value; } }",
        "struct Patch {} trait Grant { fn grant(self); } #[provider(id = \"inventory\")] impl Grant for Patch { fn grant(self) {} }",
    ] {
        let error = compile_test_program(SourceId::new(1), source)
            .expect_err("removed declarations cannot produce hidden or ordinary functions");
        let diagnostics = error.into_semantic_diagnostics();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_deref() == Some("hir::removed_declaration")
                && diagnostic.span.is_some()
        }));
    }
}

#[test]
fn compiler_rejects_removed_dispatch_without_a_service_schema() {
    for expression in [
        "service::base::grant(value)",
        "service::pinned::audit::record(value)",
        "service::pinned::missing::record(value)",
        "service::base::grant",
        "service::pinned::audit::record",
        "(|| service::base::grant(value))()",
    ] {
        let source = format!("fn main(value) {{ return {expression}; }}");
        let error = compile_test_program(SourceId::new(2), &source)
            .expect_err("removed dispatch cannot reach native lowering");
        let diagnostics = error.into_semantic_diagnostics();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_deref() == Some("hir::removed_service_call")
                && diagnostic.span.is_some()
        }));
    }
}

#[test]
fn ordinary_methods_and_traits_compile_without_service_metadata() {
    let program = compile_test_program(
        SourceId::new(3),
        r#"
struct Inventory { value: i64 }
trait Grant { fn grant(self, base: i64) -> i64; }
impl Grant for Inventory {
    fn grant(self, base: i64) -> i64 { return self.value + base; }
}
impl Inventory {
    fn record(self, services: i64) -> i64 { return self.value + services; }
}
fn main() {
    let base = Inventory { value: 2 };
    let services = Inventory { value: 3 };
    return base.grant(services.record(1));
}
"#,
    )
    .expect("ordinary trait and inherent methods compile");
    assert!(program.function("main").is_some());
    assert!(program.function("__impl.Inventory.record").is_some());
    assert!(
        program
            .function("__impl.Grant.for.Inventory.grant")
            .is_some()
    );
    assert!(program.package_metadata().is_none());
    assert_eq!(program.functions().count(), 3);
}
