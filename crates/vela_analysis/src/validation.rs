//! Executable-qualified semantic capability and use-site validation facts.
//!
//! These facts are derived after type and target inference reaches its fixed
//! point. They deliberately stay separate from compile-target validation:
//! every diagnostic here is provable from Heavy HIR plus analysis facts alone.

use std::collections::{BTreeMap, BTreeSet};

use vela_common::{Diagnostic, Span};
use vela_hir::body::HirBinaryOp;
use vela_hir::ids::{HirBodyId, HirExprId, HirStmtId};
use vela_hir::module_graph::ModuleGraph;

use crate::facts::AnalysisFacts;
use crate::registry::RegistryFacts;
use crate::semantic_facts::ConstructorTargetFact;
use crate::type_fact::TypeFact;

mod calls;
mod capabilities;
mod constructors;
mod diagnostics;

#[cfg(test)]
mod call_tests;
#[cfg(test)]
mod constructor_tests;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallPlacementModeFact {
    Strict,
    ExternalNamed,
    ExternalPositional,
    Dynamic,
    Positional,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallSourceArgumentFact {
    pub source_index: usize,
    pub name: Option<String>,
    pub value: Option<HirExprId>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallParameterSlotValueFact {
    Explicit {
        source_index: usize,
        value: Option<HirExprId>,
    },
    MissingDefault,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallParameterSlotFact {
    pub parameter_index: usize,
    pub name: String,
    pub value: CallParameterSlotValueFact,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallArgumentPlacementFact {
    pub mode: CallPlacementModeFact,
    pub source_order: Vec<CallSourceArgumentFact>,
    pub parameter_slots: Option<Vec<CallParameterSlotFact>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructorInputKindFact {
    RecordFields,
    TupleArguments,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructorSourceValueFact {
    pub source_index: usize,
    pub name: Option<String>,
    pub value: Option<HirExprId>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstructorSlotValueFact {
    Explicit {
        source_index: usize,
        value: Option<HirExprId>,
    },
    SourceDefault {
        body: HirBodyId,
    },
    /// HIR declared a source default but did not retain a complete provider.
    SourceDefaultUnavailable {
        body: Option<HirBodyId>,
    },
    /// Registry metadata promises a default but does not carry its value.
    RegisteredDefaultUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructorFieldSlotFact {
    pub declaration_index: usize,
    pub field_name: String,
    pub parameter_name: String,
    pub expected: TypeFact,
    pub declaration_span: Option<Span>,
    pub value: ConstructorSlotValueFact,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructorPlacementFact {
    pub target: ConstructorTargetFact,
    pub input_kind: ConstructorInputKindFact,
    pub source_order: Vec<ConstructorSourceValueFact>,
    pub declaration_slots: Option<Vec<ConstructorFieldSlotFact>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExecutableValidationFacts {
    operators: BTreeMap<HirExprId, OperatorCapabilityFact>,
    array_ordering: BTreeMap<HirExprId, ArrayOrderingCapabilityFact>,
    loop_controls: BTreeMap<HirStmtId, LoopControlFact>,
    calls: BTreeMap<HirExprId, CallArgumentPlacementFact>,
    constructors: BTreeMap<HirExprId, ConstructorPlacementFact>,
    call_diagnostic_batches: Vec<(HirExprId, Span, Vec<Diagnostic>)>,
    constructor_diagnostic_batches: Vec<(HirExprId, Span, Vec<Diagnostic>)>,
    diagnostics: Vec<Diagnostic>,
}

impl ExecutableValidationFacts {
    pub(crate) fn from_analysis(
        graph: &ModuleGraph,
        schema: Option<&RegistryFacts>,
        facts: &AnalysisFacts,
        bodies: &BTreeSet<HirBodyId>,
    ) -> Self {
        let capabilities = CapabilityIndex::new(graph);
        let mut validation = Self::default();
        for body in bodies.iter().filter_map(|body| graph.body(*body)) {
            capabilities::record_body(&mut validation, &capabilities, graph, facts, body);
            calls::record_body(&mut validation, graph, schema, facts, body);
            constructors::record_body(&mut validation, graph, schema, facts, body);
        }
        let constructor_diagnostic_expressions = validation
            .constructor_diagnostic_batches
            .iter()
            .map(|(expression, _, _)| *expression)
            .collect::<BTreeSet<_>>();
        let mut diagnostic_batches = validation
            .diagnostics
            .drain(..)
            .map(|diagnostic| (diagnostic.span, vec![diagnostic]))
            .chain(
                validation
                    .call_diagnostic_batches
                    .drain(..)
                    .filter(|(expression, _, _)| {
                        !constructor_diagnostic_expressions.contains(expression)
                    })
                    .map(|(_, origin, diagnostics)| (Some(origin), diagnostics)),
            )
            .chain(
                validation
                    .constructor_diagnostic_batches
                    .drain(..)
                    .map(|(_, origin, diagnostics)| (Some(origin), diagnostics)),
            )
            .collect::<Vec<_>>();
        diagnostic_batches
            .sort_by_key(|(origin, _)| origin.map(|span| (span.source, span.start, span.end)));
        validation.diagnostics = diagnostic_batches
            .into_iter()
            .flat_map(|(_, diagnostics)| diagnostics)
            .collect();
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
    pub fn call_argument_placement(
        &self,
        expression: HirExprId,
    ) -> Option<&CallArgumentPlacementFact> {
        self.calls.get(&expression)
    }

    #[must_use]
    pub fn constructor_placement(
        &self,
        expression: HirExprId,
    ) -> Option<&ConstructorPlacementFact> {
        self.constructors.get(&expression)
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}
