use vela_def::{DefPath, FunctionId};
use vela_registry::{
    DefinitionRegistry, EffectSet, FunctionAccessDef, FunctionDef, FunctionSignature, ParamDef,
    RegistryError, TypeHintDef,
};

/// Backend-neutral metadata for one policy-controlled reflection native.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReflectionNativeSpec {
    pub source_name: &'static str,
    pub params: &'static [&'static str],
    pub operation: ReflectionNativeOperation,
    pub effects: EffectSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReflectionNativeOperation {
    Read,
    Write,
    Call,
}

impl ReflectionNativeSpec {
    #[must_use]
    pub const fn read(source_name: &'static str, params: &'static [&'static str]) -> Self {
        Self {
            source_name,
            params,
            operation: ReflectionNativeOperation::Read,
            effects: EffectSet {
                reflection_read: true,
                host_read: false,
                host_write: false,
                reflection_write: false,
                reflection_call: false,
                event_emit: false,
                time: false,
                random: false,
                io_read: false,
                io_write: false,
            },
        }
    }

    #[must_use]
    pub const fn write(source_name: &'static str, params: &'static [&'static str]) -> Self {
        Self {
            source_name,
            params,
            operation: ReflectionNativeOperation::Write,
            effects: EffectSet {
                reflection_read: false,
                host_read: false,
                host_write: false,
                reflection_write: true,
                reflection_call: false,
                event_emit: false,
                time: false,
                random: false,
                io_read: false,
                io_write: false,
            },
        }
    }

    #[must_use]
    pub const fn call(source_name: &'static str, params: &'static [&'static str]) -> Self {
        Self {
            source_name,
            params,
            operation: ReflectionNativeOperation::Call,
            effects: EffectSet {
                reflection_read: false,
                host_read: false,
                host_write: false,
                reflection_write: false,
                reflection_call: true,
                event_emit: false,
                time: false,
                random: false,
                io_read: false,
                io_write: false,
            },
        }
    }

    #[must_use]
    pub fn path(self) -> DefPath {
        let mut parts = self
            .source_name
            .split("::")
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        let function_name = parts.pop().unwrap_or(self.source_name);
        DefPath::function("host", parts, function_name)
    }

    #[must_use]
    pub fn id(self) -> FunctionId {
        FunctionId::from_def_id(self.path().id())
    }

    #[must_use]
    pub fn signature(self) -> FunctionSignature {
        FunctionSignature::new(
            self.params
                .iter()
                .map(|param| ParamDef::new(*param, None::<TypeHintDef>)),
            None::<TypeHintDef>,
        )
    }

    #[must_use]
    pub fn def(self) -> FunctionDef {
        FunctionDef::new(self.path(), self.signature())
            .effects(self.effects)
            .access(FunctionAccessDef::new())
    }
}

/// Reflection natives are deliberately separate from the standard registry.
/// Embedders install them only when their reflection policy enables the API.
pub const REFLECTION_NATIVES: &[ReflectionNativeSpec] = &[
    ReflectionNativeSpec::read("reflect::access", &["target"]),
    ReflectionNativeSpec::read("reflect::attr", &["target", "name"]),
    ReflectionNativeSpec::read("reflect::attrs", &["target"]),
    ReflectionNativeSpec::call("reflect::call", &["target"]),
    ReflectionNativeSpec::read("reflect::docs", &["target"]),
    ReflectionNativeSpec::read("reflect::effects", &["target"]),
    ReflectionNativeSpec::read("reflect::exports", &["target"]),
    ReflectionNativeSpec::read("reflect::field", &["target", "name"]),
    ReflectionNativeSpec::read("reflect::fields", &["target"]),
    ReflectionNativeSpec::read("reflect::function", &["name"]),
    ReflectionNativeSpec::read("reflect::functions", &[]),
    ReflectionNativeSpec::read("reflect::get", &["target", "field"]),
    ReflectionNativeSpec::read("reflect::has_attr", &["target", "name"]),
    ReflectionNativeSpec::read("reflect::has_field", &["target", "name"]),
    ReflectionNativeSpec::read("reflect::has_function", &["name"]),
    ReflectionNativeSpec::read("reflect::has_method", &["target", "name"]),
    ReflectionNativeSpec::read("reflect::has_module", &["name"]),
    ReflectionNativeSpec::read("reflect::has_permission", &["name"]),
    ReflectionNativeSpec::read("reflect::has_trait", &["name"]),
    ReflectionNativeSpec::read("reflect::has_type", &["name"]),
    ReflectionNativeSpec::read("reflect::has_variant", &["target", "name"]),
    ReflectionNativeSpec::read("reflect::id", &["target"]),
    ReflectionNativeSpec::read("reflect::implements", &["target", "trait"]),
    ReflectionNativeSpec::read("reflect::kind", &["target"]),
    ReflectionNativeSpec::read("reflect::method", &["target", "name"]),
    ReflectionNativeSpec::read("reflect::methods", &["target"]),
    ReflectionNativeSpec::read("reflect::module", &["name"]),
    ReflectionNativeSpec::read("reflect::modules", &[]),
    ReflectionNativeSpec::read("reflect::name", &["target"]),
    ReflectionNativeSpec::read("reflect::origin", &["target"]),
    ReflectionNativeSpec::read("reflect::owner", &["target"]),
    ReflectionNativeSpec::read("reflect::params", &["target"]),
    ReflectionNativeSpec::read("reflect::permissions", &[]),
    ReflectionNativeSpec::read("reflect::required_permissions", &["target"]),
    ReflectionNativeSpec::read("reflect::returns", &["target"]),
    ReflectionNativeSpec::write("reflect::set", &["target", "field", "value"]),
    ReflectionNativeSpec::read("reflect::source_span", &["target"]),
    ReflectionNativeSpec::read("reflect::trait_info", &["name"]),
    ReflectionNativeSpec::read("reflect::traits", &["target"]),
    ReflectionNativeSpec::read("reflect::type_info", &["name"]),
    ReflectionNativeSpec::read("reflect::type_of", &["target"]),
    ReflectionNativeSpec::read("reflect::types", &[]),
    ReflectionNativeSpec::read("reflect::variant", &["target"]),
    ReflectionNativeSpec::read("reflect::variant_info", &["target", "name"]),
    ReflectionNativeSpec::read("reflect::variant_is", &["target", "name"]),
    ReflectionNativeSpec::read("reflect::variants", &["target"]),
];

pub fn reflection_native_specs() -> std::slice::Iter<'static, ReflectionNativeSpec> {
    REFLECTION_NATIVES.iter()
}

pub fn reflection_native_spec(source_name: &str) -> Option<&'static ReflectionNativeSpec> {
    REFLECTION_NATIVES
        .iter()
        .find(|spec| spec.source_name == source_name)
}

pub fn register_reflection_natives(
    registry: &mut DefinitionRegistry,
) -> Result<usize, RegistryError> {
    for spec in reflection_native_specs().copied() {
        registry.register_function(spec.def())?;
    }
    Ok(REFLECTION_NATIVES.len())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use vela_def::DefPath;
    use vela_registry::{Def, DefinitionRegistry, RegistryError};

    use super::*;
    use crate::standard_registry;

    #[test]
    fn manifest_owns_names_parameters_ids_and_effects() {
        assert_eq!(REFLECTION_NATIVES.len(), 46);
        assert_eq!(
            reflection_native_specs()
                .map(|spec| spec.source_name)
                .collect::<BTreeSet<_>>()
                .len(),
            REFLECTION_NATIVES.len()
        );

        let get = reflection_native_specs()
            .copied()
            .find(|spec| spec.source_name == "reflect::get")
            .expect("reflect::get spec");
        assert_eq!(reflection_native_spec("reflect::get"), Some(&get));
        assert_eq!(reflection_native_spec("reflect::missing"), None);
        assert_eq!(get.params, ["target", "field"]);
        assert_eq!(get.path(), DefPath::function("host", ["reflect"], "get"));
        assert_eq!(get.id().def_id(), get.path().id());
        assert_eq!(get.operation, ReflectionNativeOperation::Read);
        assert!(get.effects.reflection_read);

        let set = reflection_native_specs()
            .copied()
            .find(|spec| spec.source_name == "reflect::set")
            .expect("reflect::set spec");
        assert!(set.effects.reflection_write);
        assert!(!set.effects.reflection_read);

        let call = reflection_native_specs()
            .copied()
            .find(|spec| spec.source_name == "reflect::call")
            .expect("reflect::call spec");
        assert!(call.effects.reflection_call);
        assert!(!call.effects.reflection_read);

        for spec in reflection_native_specs() {
            assert_eq!(
                (
                    spec.effects.reflection_read,
                    spec.effects.reflection_write,
                    spec.effects.reflection_call,
                ),
                match spec.operation {
                    ReflectionNativeOperation::Read => (true, false, false),
                    ReflectionNativeOperation::Write => (false, true, false),
                    ReflectionNativeOperation::Call => (false, false, true),
                }
            );
        }
    }

    #[test]
    fn explicit_registration_preserves_manifest_definitions() {
        let mut registry = DefinitionRegistry::new();
        assert_eq!(
            register_reflection_natives(&mut registry).expect("reflection registration"),
            REFLECTION_NATIVES.len()
        );
        let definitions = registry.compile_view().definitions().collect::<Vec<_>>();

        for spec in reflection_native_specs() {
            let function = definitions
                .iter()
                .find_map(|definition| match definition {
                    Def::Function(function) if function.id == spec.id() => Some(function),
                    _ => None,
                })
                .expect("registered reflection function");
            assert_eq!(function.path, spec.path());
            assert_eq!(
                function
                    .signature
                    .params
                    .iter()
                    .map(|param| param.name.as_str())
                    .collect::<Vec<_>>(),
                spec.params
            );
            assert_eq!(function.effects, spec.effects);
            assert!(function.access.public);
            assert!(function.access.reflect_visible);
            assert!(!function.access.reflect_callable);
        }
    }

    #[test]
    fn reflection_natives_remain_policy_controlled_and_duplicate_safe() {
        let standard = standard_registry().expect("standard registry");
        for spec in reflection_native_specs() {
            assert_eq!(standard.id_for_path(&spec.path()), None);
        }

        let mut registry = DefinitionRegistry::new();
        register_reflection_natives(&mut registry).expect("first registration");
        let error = register_reflection_natives(&mut registry)
            .expect_err("duplicate reflection registration should fail");
        assert!(matches!(error, RegistryError::DuplicatePath { .. }));
    }
}
