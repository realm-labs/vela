#[test]
fn removed_task_path_matrix_has_no_builtin_tooling() {
    crate::removed_execution_tests::verify_fixture("completion-task-paths");
}
