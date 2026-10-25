use serde::{Deserialize, Serialize};
use vela_analysis::registry::RegistryTypeBindingFact;
use vela_common::{InteropTypeId, TypeAbiFingerprint, TypeBindingRegistryChecksum};

use super::{SchemaArtifactError, SchemaArtifactFacts};

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaTypeBindingFact {
    name: String,
    id: String,
    abi_fingerprint: String,
}

impl SchemaTypeBindingFact {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn abi_fingerprint(&self) -> &str {
        &self.abi_fingerprint
    }
}

pub(super) fn type_binding_to_schema(
    (name, binding): (&str, &RegistryTypeBindingFact),
) -> SchemaTypeBindingFact {
    SchemaTypeBindingFact {
        name: name.to_owned(),
        id: format!("0x{:032x}", binding.id.get()),
        abi_fingerprint: format!("0x{:016x}", binding.abi_fingerprint.get()),
    }
}

pub(super) fn type_binding_from_schema(
    binding: &SchemaTypeBindingFact,
) -> Option<(String, RegistryTypeBindingFact)> {
    Some((
        binding.name.clone(),
        RegistryTypeBindingFact {
            id: InteropTypeId::new(parse_integer(
                &binding.id,
                u128::from_str_radix,
                str::parse,
            )?),
            abi_fingerprint: TypeAbiFingerprint::new(parse_integer(
                &binding.abi_fingerprint,
                u64::from_str_radix,
                str::parse,
            )?),
        },
    ))
}

pub(super) fn type_binding_checksum_to_schema(checksum: TypeBindingRegistryChecksum) -> String {
    format!("0x{:016x}", checksum.get())
}

pub(super) fn type_binding_checksum_from_schema(
    checksum: &str,
) -> Option<TypeBindingRegistryChecksum> {
    parse_integer(checksum, u64::from_str_radix, str::parse).map(TypeBindingRegistryChecksum::new)
}

pub(super) fn validate_type_binding_facts(
    facts: &SchemaArtifactFacts,
) -> Result<(), SchemaArtifactError> {
    if facts
        .type_binding_checksum
        .as_deref()
        .is_some_and(|checksum| type_binding_checksum_from_schema(checksum).is_none())
    {
        return Err(SchemaArtifactError::new(
            "typeBindingChecksum must be a u64 integer string",
        ));
    }
    for binding in &facts.type_bindings {
        if binding.name.trim().is_empty() {
            return Err(SchemaArtifactError::new(
                "TypeBinding name must be non-empty",
            ));
        }
        if type_binding_from_schema(binding).is_none() {
            return Err(SchemaArtifactError::new(format!(
                "TypeBinding `{}` has invalid identity or ABI metadata",
                binding.name
            )));
        }
    }
    Ok(())
}

fn parse_integer<T>(
    value: &str,
    from_hex: impl FnOnce(&str, u32) -> Result<T, std::num::ParseIntError>,
    from_decimal: impl FnOnce(&str) -> Result<T, std::num::ParseIntError>,
) -> Option<T> {
    let value = value.trim();
    match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(hex) => from_hex(hex, 16).ok(),
        None => from_decimal(value).ok(),
    }
}
