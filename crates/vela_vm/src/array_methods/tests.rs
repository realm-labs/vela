use vela_bytecode::compiler::compile_function_source;
use vela_common::SourceId;

use crate::owned_value::OwnedValue;
use crate::{ExecutionBudget, Vm, VmErrorKind};

mod aggregation_and_ordering;
mod higher_order_and_mutation;
mod lookup_and_transform;
