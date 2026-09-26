#[test]
fn top_level_declarations_and_duplicates_have_exact_streams_deltas_and_ranges() {
    super::support::assert_fixture("semantic-token-top-level");
}
