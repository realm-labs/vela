use vela_def::{DefPath, script_type_path, script_variant_path};
use vela_registry::Def;

use super::{
    FunctionId, GuardContext, HostMethodId, HostTargetPlanId, LinkContext, LinkError,
    LinkedCodeObject, LinkedMethodDispatch, LinkedMethodDispatchKind, LinkedNativeFunction,
    LinkedType, LinkedVariant, MethodDispatchHandle, MethodDispatchKey, MethodId, NativeHandle,
    ScriptFunctionHandle, TypeGuard, TypeGuardPlan, TypeHandle, TypeId, VariantHandle, VariantId,
};
use crate::{UnlinkedCodeObject, UnlinkedTypeGuard, UnlinkedTypeGuardPlan};

impl LinkContext<'_, '_> {
    pub(super) fn resolve_script_function(
        &self,
        target: FunctionId,
        name: &str,
    ) -> Result<ScriptFunctionHandle, LinkError> {
        self.script_functions_by_id
            .get(&target)
            .copied()
            .ok_or_else(|| LinkError::MissingScriptFunction {
                name: name.to_owned(),
                id: target,
            })
    }

    pub(super) fn link_native(
        &mut self,
        name: &str,
        id: FunctionId,
    ) -> Result<NativeHandle, LinkError> {
        if let Some(handle) = self.native_handles.get(&id).copied() {
            return Ok(handle);
        }

        if let Some(registry) = self.linker.registry
            && registry.get(id.def_id()).and_then(Def::function_id) != Some(id)
        {
            return Err(LinkError::UnresolvedNative {
                name: name.to_owned(),
                id,
            });
        }

        if !self.linker.native_implementations.contains(&id) {
            return Err(LinkError::MissingNativeImplementation {
                name: name.to_owned(),
                id,
            });
        }

        let asyncness = self
            .linker
            .registry
            .and_then(|registry| registry.get(id.def_id()))
            .and_then(Def::function_signature)
            .map_or(vela_common::CallableAsyncness::Sync, |signature| {
                signature.asyncness
            });
        let debug_name = self.linked.intern_debug_name(name.to_owned());
        let handle = self.linked.push_native_function(
            LinkedNativeFunction::new(id, debug_name).with_asyncness(asyncness),
        );
        self.native_handles.insert(id, handle);
        Ok(handle)
    }

    pub(super) fn link_method_dispatch(
        &mut self,
        method: &str,
        method_id: MethodId,
    ) -> Result<MethodDispatchHandle, LinkError> {
        let key = if let Some(function) = self.script_methods_by_id.get(&method_id).copied() {
            MethodDispatchKey::Script(method_id, function)
        } else {
            if let Some(registry) = self.linker.registry
                && registry.get(method_id.def_id()).and_then(Def::method_id) != Some(method_id)
                && !vela_stdlib::STD_METHODS
                    .iter()
                    .any(|method| method.id() == method_id)
            {
                return Err(LinkError::MissingMethodDefinition {
                    method: method.to_owned(),
                    id: method_id,
                });
            }
            MethodDispatchKey::Value(method_id)
        };

        self.intern_method_dispatch(key, method.to_owned())
    }

    pub(super) fn link_host_method(&mut self, method_id: HostMethodId) -> MethodDispatchHandle {
        self.intern_method_dispatch(
            MethodDispatchKey::Host(method_id),
            format!("host_method::{}", method_id.get()),
        )
        .expect("host method dispatch cannot fail")
    }

    pub(super) fn intern_method_dispatch(
        &mut self,
        key: MethodDispatchKey,
        debug_text: String,
    ) -> Result<MethodDispatchHandle, LinkError> {
        if let Some(handle) = self.method_handles.get(&key).copied() {
            return Ok(handle);
        }

        let debug_name = self.linked.intern_debug_name(debug_text);
        let asyncness = match key {
            MethodDispatchKey::Script(_, function) => self
                .linked
                .function(function)
                .map_or(vela_common::CallableAsyncness::Sync, |code| code.asyncness),
            MethodDispatchKey::Value(method_id) => self
                .linker
                .registry
                .and_then(|registry| registry.get(method_id.def_id()))
                .and_then(Def::method_signature)
                .map_or(vela_common::CallableAsyncness::Sync, |signature| {
                    signature.asyncness
                }),
            MethodDispatchKey::Host(_) => vela_common::CallableAsyncness::Sync,
        };
        let kind = match key {
            MethodDispatchKey::Script(method_id, function) => LinkedMethodDispatchKind::Script {
                method_id,
                function,
            },
            MethodDispatchKey::Value(method_id) => LinkedMethodDispatchKind::Value { method_id },
            MethodDispatchKey::Host(method_id) => LinkedMethodDispatchKind::Host { method_id },
        };
        let handle = self.linked.push_method_dispatch(
            LinkedMethodDispatch::new(debug_name, kind).with_asyncness(asyncness),
        );
        self.method_handles.insert(key, handle);
        Ok(handle)
    }

    pub(super) fn link_type(
        &mut self,
        name: &str,
        resolved: Option<TypeId>,
    ) -> Result<TypeHandle, LinkError> {
        let id = self.resolve_type_id(name, resolved)?;
        if let Some(handle) = self.type_handles.get(&id).copied() {
            return Ok(handle);
        }

        let debug_name = self.linked.intern_debug_name(name.to_owned());
        let handle = self.linked.push_type(LinkedType::new(id, debug_name));
        self.type_handles.insert(id, handle);
        Ok(handle)
    }

    pub(super) fn link_variant(
        &mut self,
        enum_name: &str,
        variant: &str,
        resolved: Option<VariantId>,
        owner: TypeHandle,
    ) -> Result<VariantHandle, LinkError> {
        let id = self.resolve_variant_id(enum_name, variant, resolved)?;
        if let Some(handle) = self.variant_handles.get(&id).copied() {
            return Ok(handle);
        }

        let debug_name = self
            .linked
            .intern_debug_name(format!("{enum_name}::{variant}"));
        let handle = self
            .linked
            .push_variant(LinkedVariant::new(id, owner, debug_name));
        self.variant_handles.insert(id, handle);
        Ok(handle)
    }

    fn resolve_type_id(&self, name: &str, resolved: Option<TypeId>) -> Result<TypeId, LinkError> {
        if let Some(id) = resolved {
            return Ok(id);
        }
        if let Some(registry) = self.linker.registry {
            for path in type_path_candidates(name) {
                if let Some(id) = registry.get_by_path(&path).and_then(Def::type_id) {
                    return Ok(id);
                }
            }
        }
        Err(LinkError::UnresolvedType {
            name: name.to_owned(),
        })
    }

    fn resolve_variant_id(
        &self,
        enum_name: &str,
        variant: &str,
        resolved: Option<VariantId>,
    ) -> Result<VariantId, LinkError> {
        if let Some(id) = resolved {
            return Ok(id);
        }
        if let Some(registry) = self.linker.registry {
            for path in variant_path_candidates(enum_name, variant) {
                if let Some(id) = registry.get_by_path(&path).and_then(Def::variant_id) {
                    return Ok(id);
                }
            }
        }
        Err(LinkError::UnresolvedVariant {
            enum_name: enum_name.to_owned(),
            variant: variant.to_owned(),
        })
    }

    pub(super) fn link_host_target(
        &self,
        code: &UnlinkedCodeObject,
        host_target_map: &[HostTargetPlanId],
        target: HostTargetPlanId,
    ) -> Result<HostTargetPlanId, LinkError> {
        host_target_map
            .get(target.index())
            .copied()
            .ok_or_else(|| LinkError::InvalidHostTarget {
                function: code.name.clone(),
                target,
            })
    }

    pub(super) fn link_type_guard(
        &mut self,
        guard: UnlinkedTypeGuard,
        code: &mut LinkedCodeObject,
    ) -> Result<crate::TypeGuardPlanId, LinkError> {
        let plan = self.link_type_guard_plan(guard.plan)?;
        let context = GuardContext::new(
            guard.context.kind,
            guard.context.location,
            self.linked.intern_debug_name(guard.context.debug_name),
        );
        Ok(code.intern_type_guard(TypeGuard::new(plan, context)))
    }

    fn link_type_guard_plan(
        &mut self,
        plan: UnlinkedTypeGuardPlan,
    ) -> Result<TypeGuardPlan, LinkError> {
        match plan {
            UnlinkedTypeGuardPlan::Primitive(tag) => Ok(TypeGuardPlan::Primitive(tag)),
            UnlinkedTypeGuardPlan::Standard(guard) => Ok(TypeGuardPlan::Standard(guard)),
            UnlinkedTypeGuardPlan::Callable {
                accepts_direct_function,
                accepts_closure,
                positional_arity,
            } => Ok(TypeGuardPlan::Callable {
                accepts_direct_function,
                accepts_closure,
                positional_arity,
            }),
            UnlinkedTypeGuardPlan::Array { element } => Ok(TypeGuardPlan::Array {
                element: element
                    .map(|plan| self.link_type_guard_plan(*plan).map(Box::new))
                    .transpose()?,
            }),
            UnlinkedTypeGuardPlan::Map { key, value } => Ok(TypeGuardPlan::Map {
                key: key
                    .map(|plan| self.link_type_guard_plan(*plan).map(Box::new))
                    .transpose()?,
                value: value
                    .map(|plan| self.link_type_guard_plan(*plan).map(Box::new))
                    .transpose()?,
            }),
            UnlinkedTypeGuardPlan::Set { element } => Ok(TypeGuardPlan::Set {
                element: element
                    .map(|plan| self.link_type_guard_plan(*plan).map(Box::new))
                    .transpose()?,
            }),
            UnlinkedTypeGuardPlan::Iterator { item } => Ok(TypeGuardPlan::Iterator {
                item: item
                    .map(|plan| self.link_type_guard_plan(*plan).map(Box::new))
                    .transpose()?,
            }),
            UnlinkedTypeGuardPlan::Tuple { elements } => Ok(TypeGuardPlan::Tuple {
                elements: elements
                    .into_iter()
                    .map(|plan| {
                        plan.map(|plan| self.link_type_guard_plan(*plan).map(Box::new))
                            .transpose()
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            UnlinkedTypeGuardPlan::Option { some } => Ok(TypeGuardPlan::Option {
                some: some
                    .map(|plan| self.link_type_guard_plan(*plan).map(Box::new))
                    .transpose()?,
            }),
            UnlinkedTypeGuardPlan::Result { ok, err } => Ok(TypeGuardPlan::Result {
                ok: ok
                    .map(|plan| self.link_type_guard_plan(*plan).map(Box::new))
                    .transpose()?,
                err: err
                    .map(|plan| self.link_type_guard_plan(*plan).map(Box::new))
                    .transpose()?,
            }),
            UnlinkedTypeGuardPlan::Type { name, type_id } => {
                self.link_type(&name, type_id).map(TypeGuardPlan::Type)
            }
            UnlinkedTypeGuardPlan::Variant {
                enum_name,
                type_id,
                variant,
                variant_id,
            } => {
                let owner = self.link_type(&enum_name, type_id)?;
                self.link_variant(&enum_name, &variant, variant_id, owner)
                    .map(TypeGuardPlan::Variant)
            }
            UnlinkedTypeGuardPlan::Shape {
                type_name,
                type_id,
                shape_id,
            } => self
                .link_type(&type_name, Some(type_id))
                .map(|ty| TypeGuardPlan::Shape { ty, shape_id }),
            UnlinkedTypeGuardPlan::HostType {
                type_name,
                type_id,
                host_type_id,
            } => self
                .link_type(&type_name, Some(type_id))
                .map(|ty| TypeGuardPlan::HostType { ty, host_type_id }),
        }
    }
}

fn type_path_candidates(name: &str) -> Vec<DefPath> {
    let mut paths = Vec::new();
    if !name.contains("::") {
        paths.push(DefPath::ty("std", std::iter::empty::<&str>(), name));
        paths.push(DefPath::ty("host", std::iter::empty::<&str>(), name));
    }
    paths.push(script_type_path(
        vela_package::PackageId::anonymous().as_str(),
        name,
    ));
    paths
}

fn variant_path_candidates(enum_name: &str, variant: &str) -> Vec<DefPath> {
    let mut paths = Vec::new();
    if !enum_name.contains("::") {
        paths.push(DefPath::variant(
            "std",
            std::iter::empty::<&str>(),
            enum_name,
            variant,
        ));
        paths.push(DefPath::variant(
            "host",
            std::iter::empty::<&str>(),
            enum_name,
            variant,
        ));
    }
    paths.push(script_variant_path(
        vela_package::PackageId::anonymous().as_str(),
        enum_name,
        variant,
    ));
    paths
}
