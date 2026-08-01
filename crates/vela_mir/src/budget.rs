use std::collections::BTreeMap;

use crate::{
    MirBlockId, MirFunction, MirStatementId, MirStatementKind, MirTerminatorKind,
    verifier::cfg::FunctionGraph,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirBudgetClass {
    LoopBackedge,
    IteratorStep,
    Call,
    DynamicWork,
    Allocation,
    HostAccess,
    Reflection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirBudgetPoint {
    pub origin: crate::MirSourceOrigin,
    pub units: u32,
    pub class: MirBudgetClass,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MirBudgetSite {
    StatementBefore(MirStatementId),
    TerminatorBefore(MirBlockId),
    Edge { from: MirBlockId, to: MirBlockId },
}

impl MirBudgetPoint {
    #[must_use]
    pub const fn new(origin: crate::MirSourceOrigin, class: MirBudgetClass) -> Self {
        Self {
            origin,
            units: 1,
            class,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirBudgetSchedule {
    statement_before: BTreeMap<MirStatementId, MirBudgetPoint>,
    terminator_before: BTreeMap<MirBlockId, MirBudgetPoint>,
    edges: BTreeMap<(MirBlockId, MirBlockId), MirBudgetPoint>,
}

impl MirBudgetSchedule {
    #[must_use]
    pub fn statement_before(&self, statement: MirStatementId) -> Option<MirBudgetPoint> {
        self.statement_before.get(&statement).copied()
    }

    #[must_use]
    pub fn terminator_before(&self, block: MirBlockId) -> Option<MirBudgetPoint> {
        self.terminator_before.get(&block).copied()
    }

    #[must_use]
    pub fn edge(&self, from: MirBlockId, to: MirBlockId) -> Option<MirBudgetPoint> {
        self.edges.get(&(from, to)).copied()
    }

    #[must_use]
    pub fn point(&self, site: MirBudgetSite) -> Option<MirBudgetPoint> {
        match site {
            MirBudgetSite::StatementBefore(statement) => self.statement_before(statement),
            MirBudgetSite::TerminatorBefore(block) => self.terminator_before(block),
            MirBudgetSite::Edge { from, to } => self.edge(from, to),
        }
    }

    pub fn points(&self) -> impl Iterator<Item = (MirBudgetSite, MirBudgetPoint)> + '_ {
        self.statement_points()
            .map(|(id, point)| (MirBudgetSite::StatementBefore(id), point))
            .chain(
                self.terminator_points()
                    .map(|(id, point)| (MirBudgetSite::TerminatorBefore(id), point)),
            )
            .chain(
                self.edges
                    .iter()
                    .map(|(&(from, to), &point)| (MirBudgetSite::Edge { from, to }, point)),
            )
    }

    pub fn statement_points(&self) -> impl Iterator<Item = (MirStatementId, MirBudgetPoint)> + '_ {
        self.statement_before
            .iter()
            .map(|(id, point)| (*id, *point))
    }

    pub fn terminator_points(&self) -> impl Iterator<Item = (MirBlockId, MirBudgetPoint)> + '_ {
        self.terminator_before
            .iter()
            .map(|(id, point)| (*id, *point))
    }
}

pub(crate) fn analyze(function: &MirFunction, graph: &FunctionGraph) -> MirBudgetSchedule {
    let statement_before = function
        .statements()
        .filter_map(|(id, statement)| {
            statement_budget_class(&statement.kind)
                .map(|class| (id, MirBudgetPoint::new(statement.origin, class)))
        })
        .collect();
    let mut terminator_before = BTreeMap::new();
    let mut edges = BTreeMap::new();
    for (block_id, block) in function.blocks() {
        let Some(terminator) = block.terminator() else {
            continue;
        };
        let class = match terminator.kind {
            MirTerminatorKind::IteratorNext { .. } | MirTerminatorKind::RangeNext { .. } => {
                Some(MirBudgetClass::IteratorStep)
            }
            _ => None,
        };
        if let Some(class) = class {
            terminator_before.insert(block_id, MirBudgetPoint::new(terminator.origin, class));
        }
        for target in graph
            .successors(block_id)
            .filter(|target| graph.dominates(*target, block_id))
        {
            edges.insert(
                (block_id, target),
                MirBudgetPoint::new(terminator.origin, MirBudgetClass::LoopBackedge),
            );
        }
    }
    MirBudgetSchedule {
        statement_before,
        terminator_before,
        edges,
    }
}

fn statement_budget_class(kind: &MirStatementKind) -> Option<MirBudgetClass> {
    match kind {
        MirStatementKind::Host(_) => Some(MirBudgetClass::HostAccess),
        MirStatementKind::Reflect(_) => Some(MirBudgetClass::Reflection),
        MirStatementKind::Call(_) | MirStatementKind::Task(_) => Some(MirBudgetClass::Call),
        MirStatementKind::Allocate(_) | MirStatementKind::FormatString { .. } => {
            Some(MirBudgetClass::Allocation)
        }
        MirStatementKind::MaterializeConstant(value) if value.requires_allocation() => {
            Some(MirBudgetClass::Allocation)
        }
        MirStatementKind::Iterator(_) => Some(MirBudgetClass::Allocation),
        MirStatementKind::DynamicUnary { .. }
        | MirStatementKind::DynamicBinary { .. }
        | MirStatementKind::Index(_)
        | MirStatementKind::GuardTrap { .. } => Some(MirBudgetClass::DynamicWork),
        _ => None,
    }
}
