use crate::error::HostResult;
use crate::protocol::HostCollectionKeyRef;
use crate::target::{HostPathArg, HostPathPart, HostTargetInstance};

use super::missing_target;

pub(super) fn target_is_leaf(target: HostTargetInstance<'_>, offset: usize) -> bool {
    offset == target.plan.parts.len()
}

fn target_part(target: HostTargetInstance<'_>, offset: usize) -> HostResult<&HostPathPart> {
    target
        .plan
        .parts
        .as_slice()
        .get(offset)
        .ok_or_else(|| missing_target(target))
}

pub(super) fn target_key(
    target: HostTargetInstance<'_>,
    offset: usize,
) -> HostResult<HostCollectionKeyRef<'_>> {
    match target_part(target, offset)? {
        HostPathPart::ConstKey(key) => Ok(HostCollectionKeyRef::String(key)),
        HostPathPart::DynKey { arg } | HostPathPart::DynIndex { arg } => match target.arg(*arg) {
            Some(HostPathArg::Key(key)) => Ok(key),
            Some(HostPathArg::Index(_)) | None => Err(missing_target(target)),
        },
        HostPathPart::Field(_) | HostPathPart::VariantField(_) | HostPathPart::ConstIndex(_) => {
            Err(missing_target(target))
        }
    }
}

pub(super) fn target_index(target: HostTargetInstance<'_>, offset: usize) -> HostResult<u32> {
    match target_part(target, offset)? {
        HostPathPart::ConstIndex(index) => Ok(*index),
        HostPathPart::DynIndex { arg } | HostPathPart::DynKey { arg } => match target.arg(*arg) {
            Some(HostPathArg::Index(index)) => Ok(index),
            Some(HostPathArg::Key(HostCollectionKeyRef::I64(index))) if index >= 0 => {
                u32::try_from(index).map_err(|_| missing_target(target))
            }
            Some(HostPathArg::Key(_)) | None => Err(missing_target(target)),
        },
        HostPathPart::Field(_) | HostPathPart::VariantField(_) | HostPathPart::ConstKey(_) => {
            Err(missing_target(target))
        }
    }
}
