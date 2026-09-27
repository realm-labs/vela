#[test]
fn hover_module_matrix_preserves_import_paths_aliases_and_visibility() {
    super::matrix_tests::verify_fixture("hover-s8", 260, 1740);
    super::matrix_tests::verify_fixture("hover-s8-package", 59, 392);
}
