use crate::{
    CompileTryLayoutTarget, CompileTryTarget, MirBlockId, MirEffect, MirGuardId, MirLocalId,
    MirOperand, MirSafepointId, MirSourceOrigin, MirStatementId,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MirSwitchValue {
    Bool(bool),
    Char(char),
    Signed(i64),
    Unsigned(u64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSwitchCase {
    pub value: MirSwitchValue,
    pub target: MirBlockId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirTryContinue {
    pub layout: CompileTryLayoutTarget,
    pub block: MirBlockId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirRangeStepMode {
    I64Proven,
    DynamicInteger,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MirTerminatorKind {
    Jump(MirBlockId),
    Branch {
        condition: MirOperand,
        then_block: MirBlockId,
        else_block: MirBlockId,
    },
    Switch {
        discriminant: MirOperand,
        cases: Vec<MirSwitchCase>,
        otherwise: MirBlockId,
    },
    GuardBranch {
        value: MirOperand,
        guard: MirGuardId,
        passed: MirBlockId,
        slow: MirBlockId,
    },
    TrySwitch {
        value: MirOperand,
        target: CompileTryTarget,
        result: MirLocalId,
        continuations: Vec<MirTryContinue>,
        propagate: MirBlockId,
        invalid: MirBlockId,
        join: MirBlockId,
    },
    IteratorNext {
        iterator: MirOperand,
        item: MirLocalId,
        next: MirBlockId,
        done: MirBlockId,
    },
    RangeNext {
        cursor: MirLocalId,
        end: MirOperand,
        exhausted: MirLocalId,
        inclusive: bool,
        item: MirLocalId,
        mode: MirRangeStepMode,
        next: MirBlockId,
        done: MirBlockId,
    },
    Return(Option<MirOperand>),
    TryTypeMismatch {
        target: CompileTryTarget,
    },
    Unreachable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MirTerminator {
    pub origin: MirSourceOrigin,
    pub kind: MirTerminatorKind,
    pub effect: MirEffect,
    pub safepoint: Option<MirSafepointId>,
}

impl MirTerminator {
    #[must_use]
    pub const fn new(
        origin: MirSourceOrigin,
        kind: MirTerminatorKind,
        effect: MirEffect,
        safepoint: Option<MirSafepointId>,
    ) -> Self {
        Self {
            origin,
            kind,
            effect,
            safepoint,
        }
    }
}

impl MirTerminatorKind {
    pub(crate) const fn minimum_effect(&self) -> MirEffect {
        match self {
            Self::IteratorNext { .. } => MirEffect::dynamic_call(),
            Self::RangeNext {
                mode: MirRangeStepMode::DynamicInteger,
                ..
            }
            | Self::TryTypeMismatch { .. } => MirEffect::may_trap(),
            Self::Jump(_)
            | Self::Branch { .. }
            | Self::Switch { .. }
            | Self::GuardBranch { .. }
            | Self::TrySwitch { .. }
            | Self::RangeNext {
                mode: MirRangeStepMode::I64Proven,
                ..
            }
            | Self::Return(_)
            | Self::Unreachable => MirEffect::PURE,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MirBasicBlock {
    statements: Vec<MirStatementId>,
    terminator: Option<MirTerminator>,
}

impl MirBasicBlock {
    #[must_use]
    pub fn statements(&self) -> &[MirStatementId] {
        &self.statements
    }

    #[must_use]
    pub const fn terminator(&self) -> Option<&MirTerminator> {
        self.terminator.as_ref()
    }

    pub(crate) fn push_statement(&mut self, statement: MirStatementId) {
        self.statements.push(statement);
    }

    pub(crate) fn set_terminator(&mut self, terminator: MirTerminator) {
        self.terminator = Some(terminator);
    }
}
