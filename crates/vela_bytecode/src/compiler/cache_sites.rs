use crate::{CacheSiteId, CacheSiteKind, UnlinkedInstructionKind};

pub(super) fn cache_site_kind(kind: &UnlinkedInstructionKind) -> Option<CacheSiteKind> {
    match kind {
        UnlinkedInstructionKind::LoadGlobal { .. } => Some(CacheSiteKind::GlobalRead),
        UnlinkedInstructionKind::CallNative { .. } => Some(CacheSiteKind::NativeCall),
        UnlinkedInstructionKind::CallDynamicMethod { .. }
        | UnlinkedInstructionKind::CallMethodId { .. } => Some(CacheSiteKind::MethodCall),
        UnlinkedInstructionKind::GetRecordSlot { .. } => Some(CacheSiteKind::RecordFieldRead),
        UnlinkedInstructionKind::SetRecordSlot { .. } => Some(CacheSiteKind::RecordFieldWrite),
        UnlinkedInstructionKind::HostRead { .. } => Some(CacheSiteKind::HostPathRead),
        UnlinkedInstructionKind::HostWrite { .. } => Some(CacheSiteKind::HostPathWrite),
        UnlinkedInstructionKind::HostMutate { .. } => Some(CacheSiteKind::HostPathMutate),
        UnlinkedInstructionKind::HostRemove { .. } => Some(CacheSiteKind::HostPathRemove),
        UnlinkedInstructionKind::HostCall { .. } => Some(CacheSiteKind::HostPathCall),
        _ => None,
    }
}

pub(super) fn attach_cache_site(
    kind: UnlinkedInstructionKind,
    cache_site: CacheSiteId,
) -> UnlinkedInstructionKind {
    match kind {
        UnlinkedInstructionKind::LoadGlobal {
            dst, global, slot, ..
        } => UnlinkedInstructionKind::LoadGlobal {
            dst,
            global,
            slot,
            cache_site: Some(cache_site),
        },
        UnlinkedInstructionKind::CallNative {
            dst,
            name,
            native,
            args,
            ..
        } => UnlinkedInstructionKind::CallNative {
            dst,
            name,
            native,
            cache_site: Some(cache_site),
            args,
        },
        UnlinkedInstructionKind::HostRead {
            dst,
            root,
            target,
            dynamic_args,
            ..
        } => UnlinkedInstructionKind::HostRead {
            dst,
            root,
            target,
            dynamic_args,
            cache_site,
        },
        UnlinkedInstructionKind::HostWrite {
            root,
            target,
            dynamic_args,
            src,
            ..
        } => UnlinkedInstructionKind::HostWrite {
            root,
            target,
            dynamic_args,
            src,
            cache_site,
        },
        UnlinkedInstructionKind::HostMutate {
            root,
            target,
            dynamic_args,
            op,
            rhs,
            ..
        } => UnlinkedInstructionKind::HostMutate {
            root,
            target,
            dynamic_args,
            op,
            rhs,
            cache_site,
        },
        UnlinkedInstructionKind::HostRemove {
            root,
            target,
            dynamic_args,
            ..
        } => UnlinkedInstructionKind::HostRemove {
            root,
            target,
            dynamic_args,
            cache_site,
        },
        UnlinkedInstructionKind::HostCall {
            dst,
            root,
            target,
            dynamic_args,
            method,
            args,
            ..
        } => UnlinkedInstructionKind::HostCall {
            dst,
            root,
            target,
            dynamic_args,
            method,
            args,
            cache_site,
        },
        _ => kind,
    }
}
