#[test]
fn removed_task_path_matrix_projects_no_builtin_tooling() {
    crate::tests::removed_execution::verify_fixture("completion-task-paths");
}
