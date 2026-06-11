use vela_bytecode::{LinkedCodeObject, LinkedProgram, ProgramImage};
use vela_def::MethodId;
use vela_host::access::HostAccess;
use vela_host::adapter::ScriptStateAdapter;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_hot_reload::symbol::ProgramVersionId;
use vela_vm::error::VmResult;

use crate::engine::Engine;

use super::call_args::call_args_type_error;
use super::{
    CallArgs, CallOptions, RuntimeGlobalStore, RuntimeScriptGlobalStore, VelaValue,
    bytecode_profile::RuntimeBytecodeProfile, inline_cache::InlineCaches, unknown_function,
    unknown_method, value_type_name,
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
    pub(super) receiver_type: String,
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
    pub fn receiver_type(&self) -> &str {
        &self.receiver_type
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

pub trait RuntimeCallTarget {
    fn resolve<'program>(
        self,
        runtime_id: u64,
        program: &'program LinkedProgram,
        version_id: Option<ProgramVersionId>,
    ) -> VmResult<ResolvedRuntimeFunction<'program>>;
}

impl RuntimeCallTarget for &str {
    fn resolve<'program>(
        self,
        _runtime_id: u64,
        program: &'program LinkedProgram,
        _version_id: Option<ProgramVersionId>,
    ) -> VmResult<ResolvedRuntimeFunction<'program>> {
        let code = linked_function_by_name(program, self)?;
        Ok(ResolvedRuntimeFunction {
            name: self.to_owned(),
            code,
            params: linked_params(program, code),
            param_defaults: code.param_defaults.clone(),
        })
    }
}

impl RuntimeCallTarget for &String {
    fn resolve<'program>(
        self,
        runtime_id: u64,
        program: &'program LinkedProgram,
        version_id: Option<ProgramVersionId>,
    ) -> VmResult<ResolvedRuntimeFunction<'program>> {
        RuntimeCallTarget::resolve(self.as_str(), runtime_id, program, version_id)
    }
}

impl RuntimeCallTarget for &VelaFunction {
    fn resolve<'program>(
        self,
        runtime_id: u64,
        program: &'program LinkedProgram,
        version_id: Option<ProgramVersionId>,
    ) -> VmResult<ResolvedRuntimeFunction<'program>> {
        if self.runtime_id != runtime_id {
            return Err(call_args_type_error(
                "VelaFunction belongs to another Runtime",
            ));
        }
        let code = linked_function_by_name(program, &self.name)?;
        let (params, param_defaults) = if self.version_id == version_id {
            (self.params.clone(), self.param_defaults.clone())
        } else {
            (linked_params(program, code), code.param_defaults.clone())
        };
        Ok(ResolvedRuntimeFunction {
            name: self.name.clone(),
            code,
            params,
            param_defaults,
        })
    }
}

impl RuntimeCallTarget for VelaFunction {
    fn resolve<'program>(
        self,
        runtime_id: u64,
        program: &'program LinkedProgram,
        version_id: Option<ProgramVersionId>,
    ) -> VmResult<ResolvedRuntimeFunction<'program>> {
        (&self).resolve(runtime_id, program, version_id)
    }
}

pub trait RuntimeMethodTarget {
    fn resolve<'program>(
        self,
        context: RuntimeMethodResolveContext<'program, '_>,
    ) -> VmResult<ResolvedRuntimeMethod<'program>>;
}

#[doc(hidden)]
pub struct RuntimeMethodResolveContext<'program, 'state> {
    pub runtime_id: u64,
    pub program_image: &'state ProgramImage,
    pub linked_program: &'program LinkedProgram,
    pub version_id: Option<ProgramVersionId>,
    pub receiver: &'state VelaValue,
    pub script_globals: &'state RuntimeScriptGlobalStore,
    pub engine: &'state Engine,
}

impl RuntimeMethodTarget for &str {
    fn resolve<'program>(
        self,
        context: RuntimeMethodResolveContext<'program, '_>,
    ) -> VmResult<ResolvedRuntimeMethod<'program>> {
        let receiver_type = value_type_name(
            &context.receiver.value,
            &context.script_globals.heap,
            context.engine.registry().as_ref(),
        )
        .ok_or_else(|| unknown_method(self.to_owned()))?;
        let method = context
            .program_image
            .script_methods()
            .get(&receiver_type, self)
            .ok_or_else(|| unknown_method(self.to_owned()))?;
        let code = linked_function_by_name(context.linked_program, &method.function)?;
        Ok(ResolvedRuntimeMethod {
            name: self.to_owned(),
            code,
            params: linked_params(context.linked_program, code)
                .into_iter()
                .skip(1)
                .collect(),
            param_defaults: code.param_defaults.iter().skip(1).copied().collect(),
        })
    }
}

impl RuntimeMethodTarget for &VelaMethod {
    fn resolve<'program>(
        self,
        context: RuntimeMethodResolveContext<'program, '_>,
    ) -> VmResult<ResolvedRuntimeMethod<'program>> {
        if self.runtime_id != context.runtime_id {
            return Err(call_args_type_error(
                "VelaMethod belongs to another Runtime",
            ));
        }
        let receiver_type = value_type_name(
            &context.receiver.value,
            &context.script_globals.heap,
            context.engine.registry().as_ref(),
        )
        .ok_or_else(|| unknown_method(self.name.clone()))?;
        if receiver_type != self.receiver_type {
            return Err(call_args_type_error(
                "VelaMethod receiver type does not match value",
            ));
        }
        let method = context
            .program_image
            .script_methods()
            .get_by_id(&self.receiver_type, self.method_id)
            .ok_or_else(|| unknown_method(self.name.clone()))?;
        let code = linked_function_by_name(context.linked_program, &method.function)?;
        let (params, param_defaults) = if self.version_id == context.version_id {
            (self.params.clone(), self.param_defaults.clone())
        } else {
            (
                linked_params(context.linked_program, code)
                    .into_iter()
                    .skip(1)
                    .collect(),
                code.param_defaults.iter().skip(1).copied().collect(),
            )
        };
        Ok(ResolvedRuntimeMethod {
            name: self.name.clone(),
            code,
            params,
            param_defaults,
        })
    }
}

impl RuntimeMethodTarget for VelaMethod {
    fn resolve<'program>(
        self,
        context: RuntimeMethodResolveContext<'program, '_>,
    ) -> VmResult<ResolvedRuntimeMethod<'program>> {
        (&self).resolve(context)
    }
}

pub struct ResolvedRuntimeFunction<'program> {
    pub(super) name: String,
    pub(super) code: &'program LinkedCodeObject,
    pub(super) params: Vec<String>,
    pub(super) param_defaults: Vec<bool>,
}

pub struct ResolvedRuntimeMethod<'program> {
    pub(super) name: String,
    pub(super) code: &'program LinkedCodeObject,
    pub(super) params: Vec<String>,
    pub(super) param_defaults: Vec<bool>,
}

pub(super) struct RuntimeCallExecution<'program, 'args, 'adapter, 'access, 'state> {
    pub(super) runtime_id: u64,
    pub(super) engine: &'program Engine,
    pub(super) registry_image: &'program ProgramImage,
    pub(super) program: &'program LinkedProgram,
    pub(super) hot_reload: Option<&'program HotReloadRuntime>,
    pub(super) globals: &'program mut RuntimeGlobalStore,
    pub(super) script_globals: &'program mut RuntimeScriptGlobalStore,
    pub(super) inline_caches: &'state InlineCaches,
    pub(super) bytecode_profile: &'state RuntimeBytecodeProfile,
    pub(super) target: ResolvedRuntimeFunction<'program>,
    pub(super) args: &'adapter mut CallArgs<'args>,
    pub(super) options: CallOptions,
    pub(super) adapter: &'adapter mut dyn ScriptStateAdapter,
    pub(super) access: &'access mut HostAccess,
}

fn linked_function_by_name<'program>(
    program: &'program LinkedProgram,
    name: &str,
) -> VmResult<&'program LinkedCodeObject> {
    let function = program
        .entry_point_by_name(name)
        .ok_or_else(|| unknown_function(name.to_owned()))?;
    program
        .function(function)
        .ok_or_else(|| unknown_function(name.to_owned()))
}

fn linked_params(program: &LinkedProgram, code: &LinkedCodeObject) -> Vec<String> {
    code.params
        .iter()
        .map(|param| program.debug_name(*param).to_owned())
        .collect()
}
