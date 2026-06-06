use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, TypeId};
use vela_host::error::HostErrorKind;
use vela_host::mock::MockStateAdapter;
use vela_host::patch::PatchOp;
use vela_host::path::{HostPath, HostRef};
use vela_host::tx::PatchTx;
use vela_host::value::HostValue;
use vela_hot_reload::abi::{AccessAbi, EffectAbi, FunctionAbi, HotReloadAbi, MethodAbi};
use vela_hot_reload::compile::{compile_initial_with_abi, compile_update_with_abi};
use vela_hot_reload::error::HotReloadErrorKind;
use vela_hot_reload::module_abi::{ModuleAbi, ModuleExportAbi};
use vela_hot_reload::policy::HotReloadPolicy;
use vela_hot_reload::report::HotReloadReport;
use vela_hot_reload::report_render::HotReloadReportLineKind;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_reflect::access::{MethodAccess, MethodEffectSet};
use vela_reflect::registry::{MethodDesc, MethodParamDesc, SchemaHash, TypeDesc, TypeKey};
use vela_vm::HostExecution;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::error::EngineErrorKind;
use crate::native::{EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint};
use crate::reload::EngineHotReloadSourceErrorKind;
use crate::runtime::{CallOptions, Runtime};
use crate::source::EngineSourceErrorKind;

use super::player_type;

mod changed_file_functions;
mod changed_file_native_method;
mod changed_file_schema_trait;
mod dir_basic;
mod dir_function_abi;
mod dir_schema_trait_abi;
mod runtime_rejection_policy;
mod runtime_safe_points;
mod source_file_native_method;
mod source_file_runtime;
mod source_file_schema_trait;

include!("fixtures.rs");
include!("report_helpers.rs");
include!("function_abi_helpers.rs");
include!("host_method_and_source_helpers.rs");
