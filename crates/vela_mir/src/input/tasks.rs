use vela_def::FunctionId;
use vela_hir::ids::HirExprId;

use vela_common::Detachability;

use crate::{CompileParameter, MirTypeContract};

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
    pub detachability: CompileTaskDetachability,
    pub continuation: Option<CompileTaskContinuationTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileTaskDetachability {
    /// Worker parameter order, including slots filled by child-side defaults.
    pub parameters: Vec<Detachability>,
    pub result: Detachability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileTaskContinuationTarget {
    pub function: FunctionId,
    pub debug_name: String,
    /// Exact first-parameter ABI derived from the worker return contract.
    pub outcome_contract: MirTypeContract,
    /// Fresh host safe-point parameters after the owned outcome.
    pub resume_parameters: Vec<CompileParameter>,
}
