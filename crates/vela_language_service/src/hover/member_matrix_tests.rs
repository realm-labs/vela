#[test]
fn hover_member_matrix_preserves_constructor_labels_and_scoped_owners() {
    super::matrix_tests::verify_fixture("hover-s4", 158, 1034);
}
