use std::sync::Arc;

use vela_bytecode::{
    LinkedArtifact, LinkedCodeObject, LinkedProgram, ProgramImage, ScriptFunctionHandle,
};
use vela_common::CallableAsyncness;
use vela_def::{MethodId, TypeId};
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_hot_reload::symbol::ProgramVersionId;
use vela_vm::error::VmResult;

use crate::engine::Engine;

use super::call_args::call_args_type_error;
use super::{
    CallArgs, ProviderMethodTarget, RuntimeGlobalStore, RuntimeScriptGlobalStore, VelaValue,
    state::RuntimeSidecars, unknown_function, unknown_method, value_type_id,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VelaFunction {
    pub(super) runtime_id: u64,
    pub(super) name: String,
    pub(super) version_id: Option<ProgramVersionId>,
    pub(super) params: Vec<String>,
    pub(super) param_defaults: Vec<bool>,
}

impl VelaFunction {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn version_id(&self) -> Option<ProgramVersionId> {
        self.version_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VelaMethod {
    pub(super) runtime_id: u64,
    pub(super) receiver_type: TypeId,
    pub(super) name: String,
    pub(super) method_id: MethodId,
    pub(super) version_id: Option<ProgramVersionId>,
    pub(super) params: Vec<String>,
    pub(super) param_defaults: Vec<bool>,
}

impl VelaMethod {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn receiver_type(&self) -> TypeId {
        self.receiver_type
    }

    #[must_use]
    pub fn method_id(&self) -> MethodId {
        self.method_id
    }

    #[must_use]
    pub fn version_id(&self) -> Option<ProgramVersionId> {
        self.version_id
    }
}

#[derive(Clone, Debug)]
pub struct VelaMethodTarget {
    pub(super) runtime_id: u64,
    pub(super) receiver: VelaValue,
    pub(super) method: VelaMethod,
}

impl VelaMethodTarget {
    #[must_use]
    pub fn receiver(&self) -> &VelaValue {
        &self.receiver
    }

    #[must_use]
    pub fn method(&self) -> &VelaMethod {
        &self.method
    }
}

pub trait RuntimeCallTarget: call_target_sealed::Sealed {}

impl<T> RuntimeCallTarget for T where T: call_target_sealed::Sealed {}

pub(super) mod call_target_sealed {
    use super::{RuntimeCallTargetKind, VelaFunction, VelaMethodTarget};
    use crate::runtime::ProviderMethodTarget;

    pub trait Sealed {
        fn into_call_target(self) -> RuntimeCallTargetKind;
    }

    impl Sealed for &str {
        fn into_call_target(self) -> RuntimeCallTargetKind {
            RuntimeCallTargetKind::FunctionName(self.to_owned())
        }
    }

    impl Sealed for String {
        fn into_call_target(self) -> RuntimeCallTargetKind {
            RuntimeCallTargetKind::FunctionName(self)
        }
    }

    impl Sealed for &String {
        fn into_call_target(self) -> RuntimeCallTargetKind {
            RuntimeCallTargetKind::FunctionName(self.clone())
        }
    }

    impl Sealed for VelaFunction {
        fn into_call_target(self) -> RuntimeCallTargetKind {
            RuntimeCallTargetKind::Function(self)
        }
    }

    impl Sealed for &VelaFunction {
        fn into_call_target(self) -> RuntimeCallTargetKind {
            RuntimeCallTargetKind::Function(self.clone())
        }
    }

    impl Sealed for VelaMethodTarget {
        fn into_call_target(self) -> RuntimeCallTargetKind {
            RuntimeCallTargetKind::BoundMethod(self)
        }
    }

    impl Sealed for ProviderMethodTarget {
        fn into_call_target(self) -> RuntimeCallTargetKind {
            RuntimeCallTargetKind::ProviderMethod(self)
        }
    }
}

#[doc(hidden)]
pub enum RuntimeCallTargetKind {
    FunctionName(String),
    Function(VelaFunction),
    BoundMethod(VelaMethodTarget),
    ProviderMethod(ProviderMethodTarget),
}

pub trait RuntimeMethodSelector: method_selector_sealed::Sealed {}

impl<T> RuntimeMethodSelector for T where T: method_selector_sealed::Sealed {}

pub(super) mod method_selector_sealed {
    use super::{RuntimeMethodSelectorKind, VelaMethod};

    pub trait Sealed {
        fn into_method_selector(self) -> RuntimeMethodSelectorKind;
    }

    impl Sealed for &str {
        fn into_method_selector(self) -> RuntimeMethodSelectorKind {
            RuntimeMethodSelectorKind::Name(self.to_owned())
        }
    }

    impl Sealed for String {
        fn into_method_selector(self) -> RuntimeMethodSelectorKind {
            RuntimeMethodSelectorKind::Name(self)
        }
    }

    impl Sealed for &String {
        fn into_method_selector(self) -> RuntimeMethodSelectorKind {
            RuntimeMethodSelectorKind::Name(self.clone())
        }
    }

    impl Sealed for VelaMethod {
        fn into_method_selector(self) -> RuntimeMethodSelectorKind {
            RuntimeMethodSelectorKind::Method(self)
        }
    }

    impl Sealed for &VelaMethod {
        fn into_method_selector(self) -> RuntimeMethodSelectorKind {
            RuntimeMethodSelectorKind::Method(self.clone())
        }
    }
}

#[doc(hidden)]
pub enum RuntimeMethodSelectorKind {
    Name(String),
    Method(VelaMethod),
}

#[doc(hidden)]
pub struct RuntimeMethodResolveContext<'program, 'state> {
    pub runtime_id: u64,
    pub program_image: &'state ProgramImage,
    pub linked_program: &'program LinkedProgram,
    pub version_id: Option<ProgramVersionId>,
    pub script_globals: &'state RuntimeScriptGlobalStore,
    pub engine: &'state Engine,
}

pub(super) fn resolve_bound_method(
    target: VelaMethodTarget,
    context: RuntimeMethodResolveContext<'_, '_>,
) -> VmResult<EntryRequest> {
    if target.runtime_id != context.runtime_id {
        return Err(call_args_type_error(
            "bound Vela method belongs to another Runtime",
        ));
    }
    let method_handle = target.method;
    if method_handle.runtime_id != context.runtime_id {
        return Err(call_args_type_error(
            "VelaMethod belongs to another Runtime",
        ));
    }
    let receiver_type = value_type_id(
        &target.receiver.value,
        &context.script_globals.heap,
        context.engine.registry().as_ref(),
    )
    .ok_or_else(|| unknown_method(method_handle.name.clone()))?;
    if receiver_type != method_handle.receiver_type {
        return Err(call_args_type_error(
            "VelaMethod receiver type does not match value",
        ));
    }
    let method = context
        .program_image
        .script_methods()
        .get_by_id(method_handle.receiver_type, method_handle.method_id)
        .ok_or_else(|| unknown_method(method_handle.name.clone()))?;
    let (function, code) =
        linked_function_by_id(context.linked_program, method.function_id, &method.function)?;
    let (params, param_defaults) = if method_handle.version_id == context.version_id {
        (
            method_handle.params.clone(),
            method_handle.param_defaults.clone(),
        )
    } else {
        (
            linked_params(context.linked_program, code)
                .into_iter()
                .skip(1)
                .collect(),
            code.param_defaults.iter().skip(1).copied().collect(),
        )
    };
    Ok(EntryRequest {
        name: method_handle.name,
        asyncness: code.asyncness,
        function,
        params,
        param_defaults,
        receiver: Some(target.receiver),
    })
}

pub(super) fn resolve_function_target(
    target: RuntimeCallTargetKind,
    runtime_id: u64,
    program: &LinkedProgram,
    version_id: Option<ProgramVersionId>,
) -> VmResult<EntryRequest> {
    match target {
        RuntimeCallTargetKind::FunctionName(name) => {
            let (function, code) = linked_function_by_name(program, &name)?;
            Ok(EntryRequest {
                name,
                asyncness: code.asyncness,
                function,
                params: linked_params(program, code),
                param_defaults: code.param_defaults.clone(),
                receiver: None,
            })
        }
        RuntimeCallTargetKind::Function(function_handle) => {
            if function_handle.runtime_id != runtime_id {
                return Err(call_args_type_error(
                    "VelaFunction belongs to another Runtime",
                ));
            }
            let (function, code) = linked_function_by_name(program, &function_handle.name)?;
            let (params, param_defaults) = if function_handle.version_id == version_id {
                (function_handle.params, function_handle.param_defaults)
            } else {
                (linked_params(program, code), code.param_defaults.clone())
            };
            Ok(EntryRequest {
                name: function_handle.name,
                asyncness: code.asyncness,
                function,
                params,
                param_defaults,
                receiver: None,
            })
        }
        RuntimeCallTargetKind::BoundMethod(_) | RuntimeCallTargetKind::ProviderMethod(_) => Err(
            call_args_type_error("call target requires runtime resolution"),
        ),
    }
}

pub(super) struct EntryRequest {
    pub(super) name: String,
    pub(super) asyncness: CallableAsyncness,
    pub(super) function: ScriptFunctionHandle,
    pub(super) params: Vec<String>,
    pub(super) param_defaults: Vec<bool>,
    pub(super) receiver: Option<VelaValue>,
}

pub(super) struct RuntimeCallExecution<'program, 'state, 'host> {
    pub(super) runtime_id: u64,
    pub(super) engine: &'program Engine,
    pub(super) registry_image: &'program ProgramImage,
    pub(super) artifact: &'program Arc<LinkedArtifact>,
    pub(super) hot_reload: Option<&'program HotReloadRuntime>,
    pub(super) globals: &'state mut RuntimeGlobalStore,
    pub(super) script_globals: &'state mut RuntimeScriptGlobalStore,
    pub(super) sidecars: &'state mut RuntimeSidecars,
    pub(super) target: EntryRequest,
    pub(super) args: CallArgs<'host>,
    pub(super) budget: vela_vm::budget::ExecutionBudget,
}

fn linked_function_by_name<'program>(
    program: &'program LinkedProgram,
    name: &str,
) -> VmResult<(ScriptFunctionHandle, &'program LinkedCodeObject)> {
    let function = program
        .entry_point_by_name(name)
        .ok_or_else(|| unknown_function(name.to_owned()))?;
    program
        .function(function)
        .map(|code| (function, code))
        .ok_or_else(|| unknown_function(name.to_owned()))
}

fn linked_function_by_id<'program>(
    program: &'program LinkedProgram,
    id: vela_def::FunctionId,
    debug_name: &str,
) -> VmResult<(ScriptFunctionHandle, &'program LinkedCodeObject)> {
    let function = program
        .entry_point_by_id(id)
        .ok_or_else(|| unknown_function(debug_name.to_owned()))?;
    program
        .function(function)
        .map(|code| (function, code))
        .ok_or_else(|| unknown_function(debug_name.to_owned()))
}

fn linked_params(program: &LinkedProgram, code: &LinkedCodeObject) -> Vec<String> {
    code.params
        .iter()
        .map(|param| program.debug_name(*param).to_owned())
        .collect()
}
