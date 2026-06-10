use std::sync::Arc;

use vela_bytecode::UnlinkedInstructionKind;
use vela_bytecode::compiler::compile_program_source;
use vela_common::{HostMethodId, HostObjectId, HostTypeId, SourceId};
use vela_def::{FieldId, TypeId};
use vela_host::access::HostAccess;
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::value::HostValue;
use vela_reflect::registry::TypeKey;
use vela_vm::HostExecution;
use vela_vm::budget::ExecutionBudgetKind;
use vela_vm::error::{VmError, VmErrorKind};
use vela_vm::owned_value::OwnedValue;

use crate::args::ScriptArgsExt;
use crate::engine::Engine;
use crate::native::{EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint};
use crate::permission::Capability;
use crate::runtime::{CallOptions, Runtime};

mod budgets_and_permissions;
mod compiler_options;
mod context_host_natives;
