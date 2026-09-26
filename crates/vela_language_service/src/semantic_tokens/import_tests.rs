#[test]
fn module_imports_aliases_and_cross_file_owners_have_exact_token_streams() {
    super::test_support::assert_fixture("semantic-token-imports");
}
