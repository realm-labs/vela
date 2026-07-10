use vela_def::{DefPath, script_type_id, script_type_path, script_variant_id, script_variant_path};
use vela_hir::attributes::schema_id_attr;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_registry::Def;

use super::*;

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

        let debug_name = self.linked.intern_debug_name(name.to_owned());
        let handle = self
            .linked
            .push_native_function(LinkedNativeFunction::new(id, debug_name));
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
        let kind = match key {
            MethodDispatchKey::Script(method_id, function) => LinkedMethodDispatchKind::Script {
                method_id,
                function,
            },
            MethodDispatchKey::Value(method_id) => LinkedMethodDispatchKind::Value { method_id },
            MethodDispatchKey::Host(method_id) => LinkedMethodDispatchKind::Host { method_id },
        };
        let handle = self
            .linked
            .push_method_dispatch(LinkedMethodDispatch::new(debug_name, kind));
        self.method_handles.insert(key, handle);
        Ok(handle)
    }

    pub(super) fn link_type(&mut self, name: &str) -> Result<TypeHandle, LinkError> {
        let id = self.resolve_type_id(name);
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
        owner: TypeHandle,
    ) -> Result<VariantHandle, LinkError> {
        let id = self.resolve_variant_id(enum_name, variant);
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

    fn resolve_type_id(&self, name: &str) -> TypeId {
        if let Some(id) = self.script_type_ids.get(name) {
            return *id;
        }
        if let Some(registry) = self.linker.registry {
            for path in type_path_candidates(name) {
                if let Some(id) = registry.get_by_path(&path).and_then(Def::type_id) {
                    return id;
                }
            }
        }
        script_type_id(name, None)
    }

    fn resolve_variant_id(&self, enum_name: &str, variant: &str) -> VariantId {
        if let Some(id) = self
            .script_variant_ids
            .get(&(enum_name.to_owned(), variant.to_owned()))
        {
            return *id;
        }
        if let Some(registry) = self.linker.registry {
            for path in variant_path_candidates(enum_name, variant) {
                if let Some(id) = registry.get_by_path(&path).and_then(Def::variant_id) {
                    return id;
                }
            }
        }
        script_variant_id(enum_name, variant, None)
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
            UnlinkedTypeGuardPlan::Type(name) => self.link_type(&name).map(TypeGuardPlan::Type),
            UnlinkedTypeGuardPlan::Variant { enum_name, variant } => {
                let owner = self.link_type(&enum_name)?;
                self.link_variant(&enum_name, &variant, owner)
                    .map(TypeGuardPlan::Variant)
            }
            UnlinkedTypeGuardPlan::Shape {
                type_name,
                shape_id,
            } => self
                .link_type(&type_name)
                .map(|ty| TypeGuardPlan::Shape { ty, shape_id }),
            UnlinkedTypeGuardPlan::HostType {
                type_name,
                host_type_id,
            } => self
                .link_type(&type_name)
                .map(|ty| TypeGuardPlan::HostType { ty, host_type_id }),
        }
    }
}

pub(super) fn script_schema_identities(
    graph: &ModuleGraph,
) -> (
    BTreeMap<String, TypeId>,
    BTreeMap<(String, String), VariantId>,
) {
    let mut types = BTreeMap::new();
    let mut variants = BTreeMap::new();
    for declaration in graph.declarations() {
        if !matches!(
            declaration.kind,
            DeclarationKind::Struct | DeclarationKind::Enum
        ) {
            continue;
        }
        let type_name = graph
            .qualified_declaration_name(declaration.id)
            .expect("stored declaration has a module path");
        let explicit = schema_id_attr(graph.declaration_attrs(declaration.id)).map(u128::from);
        types.insert(type_name.clone(), script_type_id(&type_name, explicit));
        if declaration.kind == DeclarationKind::Enum
            && let Some(shape) = graph.enum_shape(declaration.id)
        {
            for variant in &shape.variants {
                variants.insert(
                    (type_name.clone(), variant.name.clone()),
                    script_variant_id(
                        &type_name,
                        &variant.name,
                        schema_id_attr(&variant.attrs).map(u128::from),
                    ),
                );
            }
        }
    }
    (types, variants)
}

fn type_path_candidates(name: &str) -> Vec<DefPath> {
    let mut paths = Vec::new();
    if !name.contains("::") {
        paths.push(DefPath::ty("std", std::iter::empty::<&str>(), name));
        paths.push(DefPath::ty("host", std::iter::empty::<&str>(), name));
    }
    paths.push(script_type_path(name));
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
    paths.push(script_variant_path(enum_name, variant));
    paths
}
