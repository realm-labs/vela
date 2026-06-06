mod permissions;
mod proxy;
mod tx;
mod write_through;

use vela_common::{FieldId, HostMethodId, HostObjectId, HostTypeId, SourceId, Span};

use crate::{
    adapter::ScriptStateAdapter,
    error::{HostError, HostErrorKind},
    mock::MockStateAdapter,
    path::{HostPath, HostRef},
    proxy::PathProxy,
    tx::{HostObjectSnapshot, PatchTx},
    value::HostValue,
};

fn player_ref(generation: u32) -> HostRef {
    HostRef::new(HostTypeId::new(1), HostObjectId::new(7), generation)
}

fn level_path() -> HostPath {
    HostPath::new(player_ref(3)).field(FieldId::new(2))
}

fn rewards_path() -> HostPath {
    HostPath::new(player_ref(3)).field(FieldId::new(3))
}

fn quest_variant_count_path() -> HostPath {
    HostPath::new(player_ref(3))
        .field(FieldId::new(4))
        .variant_field(FieldId::new(5))
}

fn test_span() -> Span {
    Span::new(SourceId::new(9), 12, 18)
}
