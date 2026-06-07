use vela_bytecode::{CodeObject, Program};
use vela_common::MethodId;
use vela_host::access::HostAccess;
use vela_host::adapter::ScriptStateAdapter;
use vela_hot_reload::runtime::HotReloadRuntime;
use vela_hot_reload::symbol::ProgramVersionId;
use vela_vm::error::VmResult;

use crate::engine::Engine;

use super::call_args::call_args_type_error;
use super::{
    CallArgs, CallOptions, RuntimeGlobalStore, RuntimeScriptGlobalStore, VelaValue,
    unknown_function, unknown_method, value_type_name,
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
        program: &'program Program,
        version_id: Option<ProgramVersionId>,
    ) -> VmResult<ResolvedRuntimeFunction<'program>>;
}

impl RuntimeCallTarget for &str {
    fn resolve<'program>(
        self,
        _runtime_id: u64,
        program: &'program Program,
        _version_id: Option<ProgramVersionId>,
    ) -> VmResult<ResolvedRuntimeFunction<'program>> {
        let code = program
            .function(self)
            .ok_or_else(|| unknown_function(self.to_owned()))?;
        Ok(ResolvedRuntimeFunction {
            name: self.to_owned(),
            code,
            params: code.params.clone(),
            param_defaults: code.param_defaults.clone(),
        })
    }
}

impl RuntimeCallTarget for &String {
    fn resolve<'program>(
        self,
        runtime_id: u64,
        program: &'program Program,
        version_id: Option<ProgramVersionId>,
    ) -> VmResult<ResolvedRuntimeFunction<'program>> {
        RuntimeCallTarget::resolve(self.as_str(), runtime_id, program, version_id)
    }
}

impl RuntimeCallTarget for &VelaFunction {
    fn resolve<'program>(
        self,
        runtime_id: u64,
        program: &'program Program,
        version_id: Option<ProgramVersionId>,
    ) -> VmResult<ResolvedRuntimeFunction<'program>> {
        if self.runtime_id != runtime_id {
            return Err(call_args_type_error(
                "VelaFunction belongs to another Runtime",
            ));
        }
        let code = program
            .function(&self.name)
            .ok_or_else(|| unknown_function(self.name.clone()))?;
        let (params, param_defaults) = if self.version_id == version_id {
            (self.params.clone(), self.param_defaults.clone())
        } else {
            (code.params.clone(), code.param_defaults.clone())
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
        program: &'program Program,
        version_id: Option<ProgramVersionId>,
    ) -> VmResult<ResolvedRuntimeFunction<'program>> {
        (&self).resolve(runtime_id, program, version_id)
    }
}

pub trait RuntimeMethodTarget {
    fn resolve(
        self,
        runtime_id: u64,
        program: &Program,
        version_id: Option<ProgramVersionId>,
        receiver: &VelaValue,
        script_globals: &RuntimeScriptGlobalStore,
        engine: &Engine,
    ) -> VmResult<ResolvedRuntimeMethod>;
}

impl RuntimeMethodTarget for &str {
    fn resolve(
        self,
        _runtime_id: u64,
        program: &Program,
        _version_id: Option<ProgramVersionId>,
        receiver: &VelaValue,
        script_globals: &RuntimeScriptGlobalStore,
        engine: &Engine,
    ) -> VmResult<ResolvedRuntimeMethod> {
        let receiver_type = value_type_name(
            &receiver.value,
            &script_globals.heap,
            engine.registry().as_ref(),
        )
        .ok_or_else(|| unknown_method(self.to_owned()))?;
        let method_id = program
            .script_method_id(&receiver_type, self)
            .ok_or_else(|| unknown_method(self.to_owned()))?;
        let code = program
            .script_method_by_id(&receiver_type, method_id)
            .ok_or_else(|| unknown_method(self.to_owned()))?;
        Ok(ResolvedRuntimeMethod {
            name: self.to_owned(),
            method_id,
            params: code.params.iter().skip(1).cloned().collect(),
            param_defaults: code.param_defaults.iter().skip(1).copied().collect(),
        })
    }
}

impl RuntimeMethodTarget for &VelaMethod {
    fn resolve(
        self,
        runtime_id: u64,
        program: &Program,
        version_id: Option<ProgramVersionId>,
        receiver: &VelaValue,
        script_globals: &RuntimeScriptGlobalStore,
        engine: &Engine,
    ) -> VmResult<ResolvedRuntimeMethod> {
        if self.runtime_id != runtime_id {
            return Err(call_args_type_error(
                "VelaMethod belongs to another Runtime",
            ));
        }
        let receiver_type = value_type_name(
            &receiver.value,
            &script_globals.heap,
            engine.registry().as_ref(),
        )
        .ok_or_else(|| unknown_method(self.name.clone()))?;
        if receiver_type != self.receiver_type {
            return Err(call_args_type_error(
                "VelaMethod receiver type does not match value",
            ));
        }
        let code = program
            .script_method_by_id(&self.receiver_type, self.method_id)
            .ok_or_else(|| unknown_method(self.name.clone()))?;
        let (params, param_defaults) = if self.version_id == version_id {
            (self.params.clone(), self.param_defaults.clone())
        } else {
            (
                code.params.iter().skip(1).cloned().collect(),
                code.param_defaults.iter().skip(1).copied().collect(),
            )
        };
        Ok(ResolvedRuntimeMethod {
            name: self.name.clone(),
            method_id: self.method_id,
            params,
            param_defaults,
        })
    }
}

impl RuntimeMethodTarget for VelaMethod {
    fn resolve(
        self,
        runtime_id: u64,
        program: &Program,
        version_id: Option<ProgramVersionId>,
        receiver: &VelaValue,
        script_globals: &RuntimeScriptGlobalStore,
        engine: &Engine,
    ) -> VmResult<ResolvedRuntimeMethod> {
        (&self).resolve(
            runtime_id,
            program,
            version_id,
            receiver,
            script_globals,
            engine,
        )
    }
}

pub struct ResolvedRuntimeFunction<'program> {
    pub(super) name: String,
    pub(super) code: &'program CodeObject,
    pub(super) params: Vec<String>,
    pub(super) param_defaults: Vec<bool>,
}

pub struct ResolvedRuntimeMethod {
    pub(super) name: String,
    pub(super) method_id: MethodId,
    pub(super) params: Vec<String>,
    pub(super) param_defaults: Vec<bool>,
}

pub(super) struct RuntimeCallExecution<'program, 'args, 'adapter, 'access> {
    pub(super) runtime_id: u64,
    pub(super) engine: &'program Engine,
    pub(super) program: &'program Program,
    pub(super) hot_reload: Option<&'program HotReloadRuntime>,
    pub(super) globals: &'program mut RuntimeGlobalStore,
    pub(super) script_globals: &'program mut RuntimeScriptGlobalStore,
    pub(super) target: ResolvedRuntimeFunction<'program>,
    pub(super) args: &'adapter mut CallArgs<'args>,
    pub(super) options: CallOptions,
    pub(super) adapter: &'adapter mut dyn ScriptStateAdapter,
    pub(super) access: &'access mut HostAccess,
}
