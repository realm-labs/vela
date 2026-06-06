use std::collections::BTreeMap;

use crate::{
    candidates::{candidate_names, ranked_candidates},
    error::{ReflectError, ReflectErrorKind, ReflectResult},
    member_records::trait_record,
    registry::TypeRegistry,
    value::ReflectValue,
};

use super::target_type;

pub fn traits(registry: &TypeRegistry, target: &ReflectValue) -> ReflectResult<ReflectValue> {
    let desc = target_type(registry, target)?;
    Ok(ReflectValue::Array(
        desc.traits.iter().map(trait_record).collect(),
    ))
}

pub fn all_traits(registry: &TypeRegistry) -> ReflectValue {
    let mut traits = BTreeMap::new();
    for desc in registry.traits() {
        traits.insert(desc.name.clone(), desc);
    }
    for desc in registry
        .types()
        .flat_map(|type_desc| type_desc.traits.iter())
    {
        traits.entry(desc.name.clone()).or_insert(desc);
    }
    ReflectValue::Array(traits.into_values().map(trait_record).collect())
}

pub fn trait_by_name(registry: &TypeRegistry, name: &str) -> ReflectResult<ReflectValue> {
    let desc = registry.trait_metadata_by_name(name).ok_or_else(|| {
        let candidates = registry.known_trait_candidates();
        let related = ranked_candidates(
            name,
            candidates
                .iter()
                .map(|(candidate, span)| (candidate.as_str(), *span)),
        );
        ReflectError::new(ReflectErrorKind::UnknownTrait {
            trait_name: name.to_owned(),
            candidates: candidate_names(&related),
            related,
        })
    })?;
    Ok(trait_record(desc))
}

pub fn has_trait(registry: &TypeRegistry, name: &str) -> bool {
    registry.trait_metadata_by_name(name).is_some()
}
