use crate::abi::*;
use crate::compile::*;
use crate::error::*;
use crate::module_abi::*;
use crate::policy::HotReloadPolicy;
use crate::report::HotReloadReport;
use crate::report_detail::HotReloadDiagnosticDetail;
use crate::report_render::{HotReloadReportLine, HotReloadReportLineKind};
use crate::runtime::HotReloadRuntime;
use crate::schema_abi::*;
use crate::symbol::ProgramVersionId;
use vela_common::{FieldId, FunctionId, HostMethodId, MethodId, SourceId, Span, TypeId, VariantId};
use vela_reflect::access::{FunctionAccess, FunctionEffectSet, MethodAccess, MethodEffectSet};
use vela_reflect::modules::{DeclOrigin, FunctionDesc, FunctionParamDesc, ModuleDesc};
use vela_reflect::registry::{
    FieldDesc, MethodDesc, MethodParamDesc, SchemaHash, TraitDesc, TraitMethodDesc, TypeDesc,
    TypeKey, TypeKind, TypeRegistry, VariantDesc,
};
use vela_vm::Vm;
use vela_vm::owned_value::OwnedValue as Value;

mod function_abi;
mod function_policy;
mod method_abi;
mod registry_manifest;
mod runtime_reports;
mod schema_abi;
mod trait_module_abi;
