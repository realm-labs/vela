use super::*;

// Pin the legacy spellings independently of the rejection implementation.
// Engine's removed_borrowing source test also covers the six collection forms
// and PathProxy; the lowercase collection names were registry aliases.
const LEGACY_HINTS: &[&str] = &[
    "PathProxy",
    "path_proxy",
    "HostRef",
    "host_ref",
    "ArrayView",
    "arrayview",
    "ArrayMut",
    "arraymut",
    "MapView",
    "mapview",
    "MapMut",
    "mapmut",
    "SetView",
    "setview",
    "SetMut",
    "setmut",
    "View",
    "view",
    "MutView",
    "mutview",
];

fn assert_removed_hint(source: &str) {
    let error = compile_test_program(SourceId::new(601), source)
        .expect_err("removed boundary hint must not lower to Any");
    let CompileErrorKind::InvalidHirGraph(diagnostics) = error.kind else {
        panic!("expected source-boundary rejection for {source}: {error:?}");
    };
    assert!(
        diagnostics
            .iter()
            .any(
                |diagnostic| diagnostic.code.as_deref() == Some("hir::removed_host_type_hint")
                    && diagnostic.span.is_some()
            ),
        "missing retired-hint diagnostic for {source}: {diagnostics:?}"
    );
}

#[test]
fn compiler_rejects_legacy_host_hints_in_every_source_annotation() {
    for hint in LEGACY_HINTS {
        for source in [
            format!("fn main(value: {hint}) {{ return value; }}"),
            format!("fn main(value: host::{hint}) {{ return value; }}"),
            format!("fn main(value: Array<{hint}>) {{ return value; }}"),
            format!("fn main(value) -> {hint} {{ return value; }}"),
            format!("fn main(value) {{ let checked: {hint} = value; return checked; }}"),
            format!("fn main() {{ return |value: {hint}| value; }}"),
            format!("struct Container {{ value: {hint} }} fn main(value) {{ return value; }}"),
            format!("state value: {hint} = (); fn main() {{ return value; }}"),
            format!("trait Reader {{ fn read(self, value: {hint}); }} fn main() {{}}"),
        ] {
            assert_removed_hint(&source);
        }
    }
}

#[test]
fn compiler_rejects_legacy_host_hint_import_and_module_aliases() {
    for hint in LEGACY_HINTS {
        for source in [
            format!("use host::{hint} as Hidden; fn main(value: Hidden) {{ return value; }}"),
            format!("use host as boundary; fn main(value: boundary::{hint}) {{ return value; }}"),
            format!(
                "use host as boundary; use boundary::{hint} as Hidden; fn main(value: Hidden) {{ return value; }}"
            ),
            format!("use std::{hint} as Hidden; fn main(value: Array<Hidden>) {{ return value; }}"),
        ] {
            assert_removed_hint(&source);
        }
    }
}

#[test]
fn compiler_keeps_declared_script_types_with_legacy_boundary_names_guarded() {
    for name in LEGACY_HINTS {
        let program = compile_test_program(SourceId::new(602), &format!(
            "struct {name} {{ value: i64 }} fn main(value: {name}) -> {name} {{ return value; }}"
        )).unwrap_or_else(|error| panic!("ordinary script type {name} rejected: {error:?}"));
        let main = program.function("main").expect("main function");
        assert_eq!(main.param_guards.len(), 1, "{name} must not become Any");
        assert!(
            main.return_guard.is_some(),
            "{name} must retain its return contract"
        );
    }
}

#[test]
fn compiler_keeps_imported_script_types_with_legacy_boundary_names_guarded() {
    for name in LEGACY_HINTS {
        for path in ["Hidden".to_owned(), format!("owned::{name}")] {
            let program = compile_test_modules(&[
                ModuleSource::new(SourceId::new(603), PackageId::anonymous(), ModulePath::from_qualified("main"), format!(
                    "use owned::{name} as Hidden; fn main(value: {path}) -> {path} {{ return value; }}"
                )),
                ModuleSource::new(SourceId::new(604), PackageId::anonymous(), ModulePath::from_qualified("owned"), format!(
                    "pub struct {name} {{ value: i64 }}"
                )),
            ]).unwrap_or_else(|error| panic!("ordinary imported script type {path} rejected: {error:?}"));
            let main = program.function("main::main").expect("main function");
            assert_eq!(main.param_guards.len(), 1, "{path} must not become Any");
            assert!(
                main.return_guard.is_some(),
                "{path} must retain its return contract"
            );
        }
    }
}
