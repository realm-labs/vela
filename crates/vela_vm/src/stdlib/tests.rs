use vela_bytecode::compiler::{compile_function_source, compile_program_source};
use vela_common::SourceId;

use crate::{ExecutionBudget, Vm, VmErrorKind};

mod core;
mod option_result;
mod option_result_chains;
