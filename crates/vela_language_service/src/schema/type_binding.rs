use serde::{Deserialize, Serialize};
use vela_analysis::registry::RegistryTypeBindingFact;
use vela_common::{
    CollectionViewCapabilities, CollectionViewKind, CollectionViewMutation, InteropTypeId,
    ReceiverCapabilities, ReceiverCapability, StoragePolicy, TypeAbiFingerprint,
    TypeBindingRegistryChecksum,
};
use vela_def::FunctionId;

use super::{SchemaArtifactError, SchemaArtifactFacts};

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaTypeBindingFact {
    name: String,
    id: String,
    storage: String,
    capabilities: Vec<String>,
    #[serde(default)]
    collection_view: Option<SchemaCollectionViewFact>,
    #[serde(default)]
    constructor_ids: Vec<String>,
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
    pub fn storage(&self) -> &str {
        &self.storage
    }

    #[must_use]
    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    #[must_use]
    pub const fn collection_view(&self) -> Option<&SchemaCollectionViewFact> {
        self.collection_view.as_ref()
    }

    #[must_use]
    pub fn constructor_ids(&self) -> &[String] {
        &self.constructor_ids
    }

    #[must_use]
    pub fn abi_fingerprint(&self) -> &str {
        &self.abi_fingerprint
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCollectionViewFact {
    kind: String,
    #[serde(default)]
    mutation: Option<String>,
    protocols: Vec<String>,
}

impl SchemaCollectionViewFact {
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn mutation(&self) -> Option<&str> {
        self.mutation.as_deref()
    }

    #[must_use]
    pub fn protocols(&self) -> &[String] {
        &self.protocols
    }
}

pub(super) fn type_binding_to_schema(
    (name, binding): (&str, &RegistryTypeBindingFact),
) -> SchemaTypeBindingFact {
    SchemaTypeBindingFact {
        name: name.to_owned(),
        id: hex_u128(binding.id.get()),
        storage: binding.storage.as_str().to_owned(),
        capabilities: receiver_capabilities(binding.capabilities)
            .map(|capability| capability.as_str().to_owned())
            .collect(),
        collection_view: binding.collection_views.map(collection_view_to_schema),
        constructor_ids: binding
            .constructor_ids
            .iter()
            .map(|id| hex_u128(id.get()))
            .collect(),
        abi_fingerprint: hex_u64(binding.abi_fingerprint.get()),
    }
}

pub(super) fn type_binding_from_schema(
    binding: &SchemaTypeBindingFact,
) -> Option<(String, RegistryTypeBindingFact)> {
    let storage = match binding.storage.as_str() {
        "value" => StoragePolicy::Value,
        "host" => StoragePolicy::Host,
        _ => return None,
    };
    let mut capabilities = ReceiverCapabilities::NONE;
    for capability in &binding.capabilities {
        capabilities = capabilities.with(receiver_capability(capability)?);
    }
    let collection_views = match binding.collection_view.as_ref() {
        Some(view) => Some(collection_view_from_schema(view)?),
        None => None,
    };
    Some((
        binding.name.clone(),
        RegistryTypeBindingFact {
            id: InteropTypeId::new(parse_u128(&binding.id)?),
            storage,
            capabilities,
            collection_views,
            constructor_ids: binding
                .constructor_ids
                .iter()
                .map(|id| parse_u128(id).map(FunctionId::new))
                .collect::<Option<Vec<_>>>()?,
            abi_fingerprint: TypeAbiFingerprint::new(parse_u64(&binding.abi_fingerprint)?),
        },
    ))
}

pub(super) fn type_binding_checksum_to_schema(checksum: TypeBindingRegistryChecksum) -> String {
    hex_u64(checksum.get())
}

pub(super) fn type_binding_checksum_from_schema(
    checksum: &str,
) -> Option<TypeBindingRegistryChecksum> {
    parse_u64(checksum).map(TypeBindingRegistryChecksum::new)
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
        validate_type_binding(binding)?;
    }
    Ok(())
}

pub(super) fn validate_type_binding(
    binding: &SchemaTypeBindingFact,
) -> Result<(), SchemaArtifactError> {
    if binding.name.trim().is_empty() {
        return Err(SchemaArtifactError::new(
            "TypeBinding name must be non-empty",
        ));
    }
    if type_binding_from_schema(binding).is_none() {
        return Err(SchemaArtifactError::new(format!(
            "TypeBinding `{}` has invalid identity, storage, capability, view, constructor, or ABI metadata",
            binding.name
        )));
    }
    Ok(())
}

fn collection_view_to_schema(view: CollectionViewCapabilities) -> SchemaCollectionViewFact {
    let kind = view.kind();
    SchemaCollectionViewFact {
        kind: kind.as_str().to_owned(),
        mutation: view.mutation().map(|mutation| mutation.as_str().to_owned()),
        protocols: protocols(kind)
            .iter()
            .map(|protocol| (*protocol).to_owned())
            .collect(),
    }
}

fn collection_view_from_schema(
    view: &SchemaCollectionViewFact,
) -> Option<CollectionViewCapabilities> {
    let kind = match view.kind.as_str() {
        "array" => CollectionViewKind::Array,
        "map" => CollectionViewKind::Map,
        "set" => CollectionViewKind::Set,
        _ => return None,
    };
    let expected_protocols = protocols(kind);
    if view
        .protocols
        .iter()
        .map(String::as_str)
        .ne(expected_protocols.iter().copied())
    {
        return None;
    }
    match view.mutation.as_deref() {
        None => Some(CollectionViewCapabilities::read_only(kind)),
        Some("fixed") => Some(CollectionViewCapabilities::mutable(
            kind,
            CollectionViewMutation::Fixed,
        )),
        Some("growable") => Some(CollectionViewCapabilities::mutable(
            kind,
            CollectionViewMutation::Growable,
        )),
        Some(_) => None,
    }
}

fn receiver_capabilities(
    capabilities: ReceiverCapabilities,
) -> impl Iterator<Item = ReceiverCapability> {
    [
        ReceiverCapability::Owned,
        ReceiverCapability::Shared,
        ReceiverCapability::Exclusive,
        ReceiverCapability::Construct,
    ]
    .into_iter()
    .filter(move |capability| capabilities.contains(*capability))
}

fn receiver_capability(value: &str) -> Option<ReceiverCapability> {
    match value {
        "owned" => Some(ReceiverCapability::Owned),
        "shared" => Some(ReceiverCapability::Shared),
        "exclusive" => Some(ReceiverCapability::Exclusive),
        "construct" => Some(ReceiverCapability::Construct),
        _ => None,
    }
}

fn protocols(kind: CollectionViewKind) -> &'static [&'static str] {
    match kind {
        CollectionViewKind::Array => &["Sequence", "Iterable"],
        CollectionViewKind::Map => &["MapLike", "Iterable"],
        CollectionViewKind::Set => &["SetLike", "Iterable"],
    }
}

fn hex_u128(value: u128) -> String {
    format!("0x{value:032x}")
}

fn hex_u64(value: u64) -> String {
    format!("0x{value:016x}")
}

fn parse_u128(value: &str) -> Option<u128> {
    parse_integer(value, u128::from_str_radix, str::parse)
}

fn parse_u64(value: &str) -> Option<u64> {
    parse_integer(value, u64::from_str_radix, str::parse)
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
