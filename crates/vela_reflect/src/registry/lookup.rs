use super::*;
use crate::candidates::{candidate_names, ranked_candidates};

pub(super) fn find_field<'a>(desc: &'a TypeDesc, field: &str) -> ReflectResult<&'a FieldDesc> {
    desc.fields
        .iter()
        .find(|candidate| candidate.name == field)
        .ok_or_else(|| {
            let related = ranked_candidates(
                field,
                desc.fields
                    .iter()
                    .map(|field| (field.name.as_str(), field.source_span)),
            );
            ReflectError::new(ReflectErrorKind::UnknownField {
                type_name: desc.key.name.clone(),
                field: field.to_owned(),
                candidates: candidate_names(&related),
                related,
            })
        })
}

pub(super) fn find_method<'a>(desc: &'a TypeDesc, method: &str) -> ReflectResult<&'a MethodDesc> {
    desc.methods
        .iter()
        .find(|candidate| candidate.name == method)
        .ok_or_else(|| {
            let related = ranked_candidates(
                method,
                desc.methods
                    .iter()
                    .map(|method| (method.name.as_str(), method.source_span)),
            );
            ReflectError::new(ReflectErrorKind::UnknownMethod {
                type_name: desc.key.name.clone(),
                method: method.to_owned(),
                candidates: candidate_names(&related),
                related,
            })
        })
}

pub(super) fn stable_trait_id(name: &str) -> TraitId {
    TraitId::new(stable_reflect_id("trait", name, ""))
}

pub(super) fn stable_reflect_id(kind: &str, owner: &str, member: &str) -> u32 {
    let mut hash = 0x811c_9dc5;
    for byte in kind
        .bytes()
        .chain([0])
        .chain(owner.bytes())
        .chain([0])
        .chain(member.bytes())
    {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    if hash == 0 { 1 } else { hash }
}
