//! Executable-qualified semantic capability and use-site validation facts.
//!
//! These facts are derived after type and target inference reaches its fixed
//! point. They deliberately stay separate from compile-target validation:
//! every diagnostic here is provable from Heavy HIR plus analysis facts alone.

use std::collections::{BTreeMap, BTreeSet};

use vela_common::Diagnostic;
use vela_hir::body::HirBinaryOp;
use vela_hir::ids::{HirBodyId, HirExprId, HirStmtId};
use vela_hir::module_graph::ModuleGraph;

use crate::facts::AnalysisFacts;

mod capabilities;
mod diagnostics;

#[cfg(test)]
mod tests;

use capabilities::CapabilityIndex;

/// Whether analysis can prove a required semantic capability at a use site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityFact {
    Supported,
    Unsupported { type_name: String },
    Dynamic,
}

impl CapabilityFact {
    #[must_use]
    pub const fn is_supported(&self) -> bool {
        matches!(self, Self::Supported)
    }

    #[must_use]
    pub const fn is_dynamic(&self) -> bool {
        matches!(self, Self::Dynamic)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BuiltinOperatorTrait {
    PartialEq,
    PartialOrd,
    Ord,
}

impl BuiltinOperatorTrait {
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::PartialEq => "PartialEq",
            Self::PartialOrd => "PartialOrd",
            Self::Ord => "Ord",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperatorCapabilityFact {
    ReferenceIdentity {
        operator: HirBinaryOp,
        lhs_expression: HirExprId,
        lhs: CapabilityFact,
        rhs_expression: HirExprId,
        rhs: CapabilityFact,
    },
    ComparisonTrait {
        operator: HirBinaryOp,
        receiver: HirExprId,
        required: BuiltinOperatorTrait,
        capability: CapabilityFact,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArrayOrderingMethod {
    Sort,
    SortBy,
    Min,
    Max,
}

impl ArrayOrderingMethod {
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Sort => "sort",
            Self::SortBy => "sort_by",
            Self::Min => "min",
            Self::Max => "max",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArrayOrderingValueKind {
    Element,
    Key,
}

impl ArrayOrderingValueKind {
    pub(crate) const fn source_name(self) -> &'static str {
        match self {
            Self::Element => "element",
            Self::Key => "key",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArrayOrderingCapabilityFact {
    pub method: ArrayOrderingMethod,
    pub value_kind: ArrayOrderingValueKind,
    pub capability: CapabilityFact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopControlKind {
    Break,
    Continue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopControlPlacement {
    InsideLoop,
    OutsideLoop,
    UnresolvedScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoopControlFact {
    pub kind: LoopControlKind,
    pub placement: LoopControlPlacement,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExecutableValidationFacts {
    operators: BTreeMap<HirExprId, OperatorCapabilityFact>,
    array_ordering: BTreeMap<HirExprId, ArrayOrderingCapabilityFact>,
    loop_controls: BTreeMap<HirStmtId, LoopControlFact>,
    diagnostics: Vec<Diagnostic>,
}

impl ExecutableValidationFacts {
    pub(crate) fn from_analysis(
        graph: &ModuleGraph,
        facts: &AnalysisFacts,
        bodies: &BTreeSet<HirBodyId>,
    ) -> Self {
        let capabilities = CapabilityIndex::new(graph);
        let mut validation = Self::default();
        for body in bodies.iter().filter_map(|body| graph.body(*body)) {
            capabilities::record_body(&mut validation, &capabilities, graph, facts, body);
        }
        validation.diagnostics.sort_by_key(|diagnostic| {
            diagnostic
                .span
                .map(|span| (span.source, span.start, span.end))
        });
        validation
    }

    #[must_use]
    pub fn operator(&self, expression: HirExprId) -> Option<&OperatorCapabilityFact> {
        self.operators.get(&expression)
    }

    #[must_use]
    pub fn array_ordering(&self, expression: HirExprId) -> Option<&ArrayOrderingCapabilityFact> {
        self.array_ordering.get(&expression)
    }

    #[must_use]
    pub fn loop_control(&self, statement: HirStmtId) -> Option<LoopControlFact> {
        self.loop_controls.get(&statement).copied()
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}
