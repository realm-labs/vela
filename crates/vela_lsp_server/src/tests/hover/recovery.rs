#[test]
fn hover_recovery_matrix_preserves_known_facts_and_null_boundaries() {
    super::matrix::verify_fixture("hover-s9", 377, 2848);
    super::recovery_states::verify(1536);
}
