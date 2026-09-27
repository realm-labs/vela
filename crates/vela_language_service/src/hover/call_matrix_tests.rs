#[test]
fn hover_call_matrix_preserves_parameter_labels_and_lexical_arguments() {
    super::matrix_tests::verify_fixture("hover-s5", 165, 1154);
}
