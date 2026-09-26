#[test]
fn body_scopes_writes_captures_and_unresolved_names_have_exact_token_streams() {
    super::test_support::assert_fixture("semantic-token-bodies");
}
