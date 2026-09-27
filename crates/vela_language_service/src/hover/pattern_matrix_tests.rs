#[test]
fn hover_pattern_matrix_preserves_bindings_variants_and_control_scopes() {
    super::matrix_tests::verify_fixture("hover-s6", 212, 1532);
}
