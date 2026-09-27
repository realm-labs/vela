#[test]
fn hover_declaration_matrix_preserves_complete_metadata_and_static_parameters() {
    super::matrix_tests::verify_fixture("hover-s1", 90, 640);
}
