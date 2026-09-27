#[test]
fn hover_type_matrix_preserves_complete_scoped_hints_and_degraded_boundaries() {
    super::matrix_tests::verify_fixture("hover-s3", 245, 1924);
}
