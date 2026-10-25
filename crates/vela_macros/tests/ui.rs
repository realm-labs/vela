#[test]
fn unified_interop_export_signature_ui() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/interop/pass/*.rs");
    cases.compile_fail("tests/ui/interop/fail/*.rs");
}

#[test]
fn removed_macros_and_borrow_apis_are_unavailable() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/removed/*.rs");
}
