#[test]
fn hover_literal_matrix_preserves_expression_facts_and_lexical_boundaries() {
    super::matrix::verify_fixture("hover-s7", 439, 2978);
}
