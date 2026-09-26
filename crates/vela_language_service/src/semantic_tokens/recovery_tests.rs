#[test]
fn malformed_neighbors_and_incomplete_contexts_have_exact_recoverable_token_streams() {
    super::test_support::assert_fixture("semantic-token-recovery");
}
