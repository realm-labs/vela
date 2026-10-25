#[test]
fn lsp_signature_help_rejects_removed_task_operands() {
    crate::tests::removed_execution::verify_fixture("completion-task-calls");
}
