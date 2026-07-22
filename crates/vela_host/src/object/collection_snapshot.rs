use crate::error::HostResult;
use crate::protocol::{HostCollectionProjection, HostCollectionSnapshot};
use crate::target::HostTargetInstance;

use super::{ScriptHostFieldAccess, ScriptHostKey, invalid_arg};

pub(super) fn snapshot_map_entries<'a, K, V>(
    entries: impl IntoIterator<Item = (&'a K, &'a V)>,
    target: HostTargetInstance<'_>,
    offset: usize,
    projection: HostCollectionProjection,
) -> HostResult<HostCollectionSnapshot>
where
    K: ScriptHostKey + 'a,
    V: ScriptHostFieldAccess + 'a,
{
    match projection {
        HostCollectionProjection::Keys => Ok(HostCollectionSnapshot::Items(
            entries
                .into_iter()
                .map(|(key, _)| key.to_host_collection_key().into_host_value())
                .collect(),
        )),
        HostCollectionProjection::Values => entries
            .into_iter()
            .map(|(_, value)| value.read_host_target_from(target, offset))
            .collect::<HostResult<Vec<_>>>()
            .map(HostCollectionSnapshot::Items),
        HostCollectionProjection::Entries => entries
            .into_iter()
            .map(|(key, value)| {
                Ok((
                    key.to_host_collection_key().into_host_value(),
                    value.read_host_target_from(target, offset)?,
                ))
            })
            .collect::<HostResult<Vec<_>>>()
            .map(HostCollectionSnapshot::Entries),
    }
}

pub(super) fn snapshot_set_values<'a, K>(
    values: impl IntoIterator<Item = &'a K>,
    projection: HostCollectionProjection,
) -> HostResult<HostCollectionSnapshot>
where
    K: ScriptHostKey + 'a,
{
    if projection == HostCollectionProjection::Entries {
        return Err(invalid_arg(projection.name()));
    }
    Ok(HostCollectionSnapshot::Items(
        values
            .into_iter()
            .map(|value| value.to_host_collection_key().into_host_value())
            .collect(),
    ))
}
