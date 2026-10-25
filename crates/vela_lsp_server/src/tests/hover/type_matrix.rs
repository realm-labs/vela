#[test]
fn hover_type_matrix_preserves_retained_hints_and_removed_view_rejections_and_degraded_boundaries()
{
    super::matrix::verify_fixture("hover-s3", 245, 1744);
}
