use vela_def::TraitId;

pub(super) fn stable_trait_id(name: &str) -> TraitId {
    TraitId::new(u128::from(vela_common::stable_id("trait", name, "")))
}
