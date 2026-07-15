use vela_bytecode::ProgramImage;
use vela_hir::module_graph::DeclarationKind;
use vela_host::error::HostResult;
use vela_host::object::ScriptHostObject;
use vela_host::path::HostRef;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use super::{IntoStateValue, RuntimeImageStorage, RuntimeImpl};

impl<I> RuntimeImpl<I>
where
    I: RuntimeImageStorage,
{
    pub fn replace_extern_state<T>(
        &mut self,
        name: impl Into<String>,
        value: T,
    ) -> HostResult<HostRef>
    where
        T: ScriptHostObject + Send + 'static,
    {
        self.state.extern_states.bind_host(name, value)
    }

    /// Stages a host object for an `extern state` declaration in a pending
    /// hot-reload generation. The binding is validated and published only if
    /// that generation is accepted at a Runtime safe point.
    pub fn stage_extern_state<T>(
        &mut self,
        name: impl Into<String>,
        value: T,
    ) -> HostResult<HostRef>
    where
        T: ScriptHostObject + Send + 'static,
    {
        self.state.extern_states.stage_host(name, value)
    }

    #[must_use]
    pub fn extern_state_ref(&self, name: &str) -> Option<HostRef> {
        self.state.extern_states.host_ref(name)
    }

    pub fn set_state(
        &mut self,
        name: impl Into<String>,
        value: impl IntoStateValue,
    ) -> VmResult<()> {
        value.set_state(self, name.into())
    }

    pub fn state(&mut self, name: &str) -> VmResult<Option<OwnedValue>> {
        self.state.vm_states.value(name)
    }

    pub fn update_state(
        &mut self,
        name: &str,
        update: impl FnOnce(&mut OwnedValue),
    ) -> VmResult<()> {
        let mut value = self.state.vm_states.value(name)?.ok_or_else(|| {
            VmError::new(VmErrorKind::MissingVmState {
                name: name.to_owned(),
            })
        })?;
        update(&mut value);
        self.set_owned_state(name.to_owned(), value)
    }

    #[cfg(feature = "serde")]
    pub fn state_as<T>(&self, name: &str) -> VmResult<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        self.state.vm_states.value_as(name)
    }

    pub(super) fn set_owned_state(&mut self, name: String, value: OwnedValue) -> VmResult<()> {
        validate_state_contract(self.image.program_image(), &name, &value)?;
        self.state.vm_states.insert(name, value)
    }
}

fn validate_state_contract(image: &ProgramImage, name: &str, value: &OwnedValue) -> VmResult<()> {
    let Some(expected) = state_contract_type(image, name) else {
        return Ok(());
    };
    if expected == "Any" || owned_value_matches_contract(value, &expected) {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::TypeContractViolation {
        expected,
        actual: owned_value_contract_type_name(value),
        debug_name: name.to_owned(),
    }))
}

fn state_contract_type(image: &ProgramImage, name: &str) -> Option<String> {
    let graph = image.script_metadata()?;
    let leaf_name = name.rsplit("::").next().unwrap_or(name);
    let mut leaf_match = None;
    for module in graph.module_ids() {
        let module_path = graph.module_path(module)?.join();
        let declarations = graph.module(module)?;
        for declaration_name in declarations.names() {
            let declaration = declarations.get(declaration_name)?;
            let metadata = graph.declaration(declaration)?;
            if metadata.kind != DeclarationKind::State {
                continue;
            }
            let symbol = if module_path.is_empty() {
                metadata.name.clone()
            } else {
                format!("{module_path}::{}", metadata.name)
            };
            if symbol == name {
                return graph
                    .state_metadata(declaration)
                    .map(|metadata| metadata.type_hint.display());
            }
            if metadata.name == leaf_name {
                let Some(metadata) = graph.state_metadata(declaration) else {
                    continue;
                };
                if leaf_match.is_some() {
                    return None;
                }
                leaf_match = Some(metadata.type_hint.display());
            }
        }
    }
    leaf_match
}

fn owned_value_matches_contract(value: &OwnedValue, expected: &str) -> bool {
    match value {
        OwnedValue::Unit => expected == "()",
        OwnedValue::Bool(_) => expected == "bool",
        OwnedValue::Char(_) => expected == "char",
        OwnedValue::Scalar(value) => value.primitive_tag().name() == expected,
        OwnedValue::String(_) => expected == "String",
        OwnedValue::Bytes(_) => expected == "Bytes",
        OwnedValue::Tuple(_) => expected == "tuple",
        OwnedValue::Array(_) => expected == "Array",
        OwnedValue::Map(_) => expected == "Map",
        OwnedValue::Set(_) => expected == "Set",
        OwnedValue::Record { type_name, .. } => type_name == expected,
        OwnedValue::Enum { enum_name, .. } => enum_name == expected,
        OwnedValue::Closure(_) => expected == "Closure",
        OwnedValue::Range(_) => expected == "Range",
        OwnedValue::Iterator(_) => expected == "Iterator",
        OwnedValue::HostRef(_) | OwnedValue::PathProxy(_) => false,
    }
}

fn owned_value_contract_type_name(value: &OwnedValue) -> String {
    match value {
        OwnedValue::Unit => "()".to_owned(),
        OwnedValue::Bool(_) => "bool".to_owned(),
        OwnedValue::Char(_) => "char".to_owned(),
        OwnedValue::Scalar(value) => value.primitive_tag().name().to_owned(),
        OwnedValue::String(_) => "String".to_owned(),
        OwnedValue::Bytes(_) => "Bytes".to_owned(),
        OwnedValue::Tuple(_) => "tuple".to_owned(),
        OwnedValue::Array(_) => "Array".to_owned(),
        OwnedValue::Map(_) => "Map".to_owned(),
        OwnedValue::Set(_) => "Set".to_owned(),
        OwnedValue::Record { type_name, .. } => type_name.clone(),
        OwnedValue::Enum { enum_name, .. } => enum_name.clone(),
        OwnedValue::Closure(_) => "Closure".to_owned(),
        OwnedValue::Range(_) => "Range".to_owned(),
        OwnedValue::HostRef(_) => "host_ref".to_owned(),
        OwnedValue::PathProxy(_) => "path_proxy".to_owned(),
        OwnedValue::Iterator(_) => "Iterator".to_owned(),
    }
}
