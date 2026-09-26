#[test]
fn literals_operators_and_interpolations_have_exact_token_streams() {
    super::support::assert_fixture("semantic-token-literals");
}
