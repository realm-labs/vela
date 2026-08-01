mod contracts;
mod descriptors;
mod host;
mod lambdas;
mod placements;
mod tasks;
mod try_targets;

use std::collections::BTreeMap;

use vela_def::{FunctionId, MethodId, TypeId};

use crate::{
    CompileFunctionClass, CompileFunctionDescriptor, CompileMethodClass, CompileMethodDescriptor,
    CompileTypeDescriptor, HostTypeTarget, MethodExecutableTarget, MirBuildError, MirSourceOrigin,
};

use super::{CompileFunctionTarget, CompileTargetSnapshot};

impl CompileTargetSnapshot {
    pub(super) fn validate(&self) -> Result<(), MirBuildError> {
        SnapshotValidator::new(self).validate()
    }
}

pub(super) struct SnapshotValidator<'a> {
    snapshot: &'a CompileTargetSnapshot,
}

impl<'a> SnapshotValidator<'a> {
    const fn new(snapshot: &'a CompileTargetSnapshot) -> Self {
        Self { snapshot }
    }

    fn validate(&self) -> Result<(), MirBuildError> {
        descriptors::validate(self)?;
        lambdas::validate(self)?;
        placements::validate(self)?;
        tasks::validate(self)?;
        try_targets::validate(self)
    }

    fn error(&self, origin: MirSourceOrigin, message: impl Into<String>) -> MirBuildError {
        MirBuildError::InconsistentInput {
            origin,
            message: message.into(),
        }
    }

    fn retained_origin<K: Ord + std::fmt::Debug>(
        &self,
        origins: &BTreeMap<K, MirSourceOrigin>,
        key: &K,
    ) -> MirSourceOrigin {
        *origins
            .get(key)
            .unwrap_or_else(|| panic!("compile-target entry {key:?} lost its insertion origin"))
    }

    fn require_root(
        &self,
        function: FunctionId,
        origin: MirSourceOrigin,
        context: &str,
    ) -> Result<&'a CompileFunctionTarget, MirBuildError> {
        self.snapshot.function(function).ok_or_else(|| {
            self.error(
                origin,
                format!(
                    "{context} references missing executable root #{}",
                    function.get()
                ),
            )
        })
    }

    fn require_function(
        &self,
        function: FunctionId,
        origin: MirSourceOrigin,
        context: &str,
    ) -> Result<&'a CompileFunctionDescriptor, MirBuildError> {
        self.snapshot.function_descriptor(function).ok_or_else(|| {
            self.error(
                origin,
                format!("{context} references missing function #{}", function.get()),
            )
        })
    }

    fn require_script_function(
        &self,
        function: FunctionId,
        origin: MirSourceOrigin,
        context: &str,
    ) -> Result<&'a CompileFunctionDescriptor, MirBuildError> {
        let descriptor = self.require_function(function, origin, context)?;
        if descriptor.class != CompileFunctionClass::Script {
            return Err(self.error(
                origin,
                format!("{context} requires script function #{}", function.get()),
            ));
        }
        Ok(descriptor)
    }

    fn require_type(
        &self,
        type_id: TypeId,
        origin: MirSourceOrigin,
        context: &str,
    ) -> Result<&'a CompileTypeDescriptor, MirBuildError> {
        self.snapshot.type_descriptor(type_id).ok_or_else(|| {
            self.error(
                origin,
                format!("{context} references missing type #{}", type_id.get()),
            )
        })
    }

    fn require_method(
        &self,
        owner: TypeId,
        method: MethodId,
        origin: MirSourceOrigin,
        context: &str,
    ) -> Result<&'a CompileMethodDescriptor, MirBuildError> {
        self.snapshot
            .method_descriptor(owner, method)
            .ok_or_else(|| {
                self.error(
                    origin,
                    format!(
                        "{context} references missing method #{} for owner #{}",
                        method.get(),
                        owner.get()
                    ),
                )
            })
    }

    fn require_script_method(
        &self,
        target: MethodExecutableTarget,
        origin: MirSourceOrigin,
        context: &str,
    ) -> Result<&'a CompileMethodDescriptor, MirBuildError> {
        let descriptor = self.require_method(target.owner, target.method, origin, context)?;
        match &descriptor.class {
            CompileMethodClass::Script { executable, .. } if *executable == target => {
                self.require_script_function(target.function, origin, context)?;
                Ok(descriptor)
            }
            CompileMethodClass::Script { executable, .. } => Err(self.error(
                origin,
                format!(
                    "{context} targets script method {target:?}, but its descriptor uses {executable:?}"
                ),
            )),
            CompileMethodClass::Host { .. }
            | CompileMethodClass::Value
            | CompileMethodClass::Registry => Err(self.error(
                origin,
                format!("{context} requires script method {target:?}"),
            )),
        }
    }

    fn require_host_type(
        &self,
        target: HostTypeTarget,
        origin: MirSourceOrigin,
        context: &str,
    ) -> Result<&'a CompileTypeDescriptor, MirBuildError> {
        host::require_host_type(self, target, origin, context)
    }
}
