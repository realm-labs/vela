mod access;
mod permissions;
mod proxy;
mod write_through;

use vela_common::{HostMethodId, HostObjectId, HostTypeId, ScalarValue, SourceId, Span};
use vela_def::FieldId;

use crate::{
    access::{HostAccess, HostObjectSnapshot},
    adapter::{GlobalBinding, ScriptStateAdapter},
    error::{HostError, HostErrorKind},
    mock::MockStateAdapter,
    object::{HostValueFrom, HostValueInto, ScriptHostFieldAccess},
    path::{HostPath, HostRef},
    proxy::PathProxy,
    resolved::HostMutationOp,
    target::{HostTargetInstance, HostTargetPlan},
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

fn target_plan(path: &HostPath) -> HostTargetPlan {
    HostTargetPlan::from(path)
}

fn target_instance<'a>(path: &HostPath, plan: &'a HostTargetPlan) -> HostTargetInstance<'a> {
    HostTargetInstance::new(path.root, plan, &[])
}

fn test_span() -> Span {
    Span::new(SourceId::new(9), 12, 18)
}
