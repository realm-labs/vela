#[test]
fn symbol_origins_collisions_and_dynamic_shadows_have_exact_token_streams() {
    super::support::assert_fixture("semantic-token-ownership");
}
