#[test]
fn unified_interop_export_signature_ui() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/interop/pass/*.rs");
    cases.compile_fail("tests/ui/interop/fail/*.rs");
}

#[test]
fn service_contract_signature_ui() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/service/pass/*.rs");
    cases.compile_fail("tests/ui/service/fail/*.rs");
}
