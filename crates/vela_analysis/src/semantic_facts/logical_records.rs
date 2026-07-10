use crate::type_fact::TypeFact;

use super::MemberTargetFact;

pub(super) fn logical_member_target(receiver: &TypeFact, name: &str) -> Option<MemberTargetFact> {
    let TypeFact::LogicalRecord(record) = receiver else {
        return None;
    };
    Some(record.field_target(name).map_or(
        MemberTargetFact::Unresolved,
        MemberTargetFact::LogicalRecordField,
    ))
}
