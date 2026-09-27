#[test]
fn hover_ownership_matrix_preserves_exact_owners_and_shadowed_facts() {
    super::matrix_tests::verify_fixture("hover-s10", 120, 862);
}
