#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MirJitIneligibility {
    Async,
    Allocation,
    Call,
    DynamicWork,
    HostAccess,
    Reflection,
    Iterator,
    MissingAnalyses,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirJitEligibility {
    reasons: Vec<MirJitIneligibility>,
    pub safepoint_count: usize,
    pub budget_point_count: usize,
}

impl MirJitEligibility {
    #[must_use]
    pub fn is_eligible(&self) -> bool {
        self.reasons.is_empty()
    }

    #[must_use]
    pub fn reasons(&self) -> &[MirJitIneligibility] {
        &self.reasons
    }
}

pub fn restricted_jit_eligibility(
    owner: &crate::OwnedVerifiedMirProgram,
    function: crate::MirFunctionId,
) -> MirJitEligibility {
    let Some(body) = owner.program().function(function) else {
        return MirJitEligibility {
            reasons: vec![MirJitIneligibility::MissingAnalyses],
            safepoint_count: 0,
            budget_point_count: 0,
        };
    };
    let Some(analyses) = owner.analyses(function) else {
        return MirJitEligibility {
            reasons: vec![MirJitIneligibility::MissingAnalyses],
            safepoint_count: body.safepoints().count(),
            budget_point_count: 0,
        };
    };
    let mut reasons = std::collections::BTreeSet::new();
    if body.asyncness() == vela_common::CallableAsyncness::Async {
        reasons.insert(MirJitIneligibility::Async);
    }
    for (_, statement) in body.statements() {
        match &statement.kind {
            crate::MirStatementKind::Allocate(_) | crate::MirStatementKind::FormatString { .. } => {
                reasons.insert(MirJitIneligibility::Allocation);
            }
            crate::MirStatementKind::MaterializeConstant(value) if value.requires_allocation() => {
                reasons.insert(MirJitIneligibility::Allocation);
            }
            crate::MirStatementKind::Call(_) => {
                reasons.insert(MirJitIneligibility::Call);
            }
            crate::MirStatementKind::DynamicUnary { .. }
            | crate::MirStatementKind::DynamicBinary { .. }
            | crate::MirStatementKind::Index(_) => {
                reasons.insert(MirJitIneligibility::DynamicWork);
            }
            crate::MirStatementKind::Host(_) => {
                reasons.insert(MirJitIneligibility::HostAccess);
            }
            crate::MirStatementKind::Reflect(_) => {
                reasons.insert(MirJitIneligibility::Reflection);
            }
            crate::MirStatementKind::Iterator(_) => {
                reasons.insert(MirJitIneligibility::Iterator);
            }
            _ => {}
        }
    }
    for (_, block) in body.blocks() {
        if matches!(
            block.terminator().map(|terminator| &terminator.kind),
            Some(crate::MirTerminatorKind::AwaitCall { .. })
        ) {
            reasons.insert(MirJitIneligibility::Async);
        }
        if matches!(
            block.terminator().map(|terminator| &terminator.kind),
            Some(crate::MirTerminatorKind::IteratorNext { .. })
                | Some(crate::MirTerminatorKind::RangeNext { .. })
        ) {
            reasons.insert(MirJitIneligibility::Iterator);
        }
    }
    MirJitEligibility {
        reasons: reasons.into_iter().collect(),
        safepoint_count: body.safepoints().count(),
        budget_point_count: analyses.budget.statement_points().count()
            + analyses.budget.terminator_points().count(),
    }
}
