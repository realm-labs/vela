use std::sync::Arc;

use vela_bytecode::UnlinkedInstructionKind;
use vela_common::SourceId;
use vela_vm::budget::ExecutionBudgetKind;
use vela_vm::error::{VmError, VmErrorKind};
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::native::{EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint};
use crate::runtime::{CallOptions, Runtime};

mod budgets_and_permissions;
mod compiler_options;
