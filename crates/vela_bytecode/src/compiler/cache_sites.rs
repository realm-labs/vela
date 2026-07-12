use crate::{CacheSiteId, CacheSiteInstruction, CacheSiteKind, UnlinkedInstructionKind};

pub(super) fn cache_site_kind(kind: &UnlinkedInstructionKind) -> Option<CacheSiteKind> {
    kind.cache_site_policy().map(|policy| policy.kind)
}

pub(super) fn attach_cache_site(
    mut kind: UnlinkedInstructionKind,
    cache_site: CacheSiteId,
) -> UnlinkedInstructionKind {
    kind.set_cache_site(cache_site);
    kind
}
