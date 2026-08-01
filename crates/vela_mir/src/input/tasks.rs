use vela_def::FunctionId;
use vela_hir::ids::HirExprId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileTaskOperation {
    SpawnScoped,
    SpawnScopedThen,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileTaskTarget {
    pub operation: CompileTaskOperation,
    pub worker_call: HirExprId,
    pub worker: FunctionId,
    pub worker_debug_name: String,
    pub continuation: Option<CompileTaskContinuationTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileTaskContinuationTarget {
    pub function: FunctionId,
    pub debug_name: String,
}
