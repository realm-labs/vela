use vela_common::{HostMethodId, HostObjectId, HostTypeId, SourceId};
use vela_def::{FieldId, TypeId};
use vela_host::access::HostAccess;
use vela_host::error::HostErrorKind;
use vela_host::mock::MockStateAdapter;
use vela_host::path::{HostPath, HostRef};
use vela_host::value::HostValue;
use vela_hot_reload::abi::{AccessAbi, EffectAbi, FunctionAbi, HotReloadAbi, MethodAbi};
use vela_hot_reload::compile::{initial_version_from_linked_artifact, update_from_linked_artifact};
use vela_hot_reload::error::HotReloadErrorKind;
use vela_hot_reload::module_abi::{ModuleAbi, ModuleExportAbi};
use vela_hot_reload::policy::HotReloadPolicy;
use vela_hot_reload::report::HotReloadReport;
use vela_hot_reload::report_render::HotReloadReportLineKind;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_reflect::access::{MethodAccess, MethodEffectSet};
use vela_reflect::registry::{MethodDesc, MethodParamDesc, SchemaHash, TypeDesc, TypeKey};
use vela_vm::HostExecution;
use vela_vm::error::VmErrorKind;
use vela_vm::owned_value::OwnedValue;

use crate::engine::Engine;
use crate::error::EngineErrorKind;
use crate::native::{EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionId, TypeHint};
use crate::permission::ExecutionProfile;
use crate::reload::EngineHotReloadSourceErrorKind;
use crate::runtime::{CallArgs, CallOptions, Runtime};
use crate::source::EngineSourceErrorKind;

use super::player_type;

fn hot_reload_result<T>(
    result: crate::reload::EngineHotReloadSourceResult<T>,
) -> vela_hot_reload::error::HotReloadResult<T> {
    match result {
        Ok(value) => Ok(value),
        Err(crate::reload::EngineHotReloadSourceError {
            kind: EngineHotReloadSourceErrorKind::HotReload(error),
        }) => Err(error),
        Err(error) => panic!("source compilation and linking must succeed: {error}"),
    }
}

fn compile_initial_with_abi(
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
) -> vela_hot_reload::error::HotReloadResult<vela_hot_reload::version::ProgramVersion> {
    let engine = Engine::builder().build().expect("test engine");
    let program = engine
        .compile_source_with_id(source, text)
        .expect("test source compiles");
    let artifact = engine
        .link_compiled_program(program)
        .expect("test source links");
    initial_version_from_linked_artifact(abi, artifact)
}

fn compile_update_with_abi(
    previous: &vela_hot_reload::version::ProgramVersion,
    source: SourceId,
    text: &str,
    abi: HotReloadAbi,
) -> vela_hot_reload::error::HotReloadResult<vela_hot_reload::version::HotUpdate> {
    let engine = Engine::builder().build().expect("test engine");
    let program = engine
        .compile_source_with_id(source, text)
        .expect("test source compiles");
    let artifact = engine
        .link_compiled_program(program)
        .expect("test source links");
    update_from_linked_artifact(previous, abi, &HotReloadPolicy::default(), artifact)
}

mod changed_file_functions;
mod changed_file_native_method;
mod changed_file_schema_trait;
mod dir_basic;
mod dir_function_abi;
mod dir_schema_trait_abi;
mod runtime_rejection_policy;
mod runtime_safe_points;
mod source_diagnostics;
mod source_file_native_method;
mod source_file_runtime;
mod source_file_schema_trait;
mod source_file_tuple_abi;

include!("fixtures.rs");
include!("report_helpers.rs");
include!("function_abi_helpers.rs");
include!("host_method_and_source_helpers.rs");
