use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

use vela_analysis::{facts::AnalysisFacts, type_fact::TypeFact};
use vela_common::{SourceId, Span};
use vela_hir::{ids::HirExprId, module_graph::ModuleGraph};

use crate::{LanguageServiceDatabases, TextRange};

pub(crate) fn collect(
    graph: &ModuleGraph,
    source: SourceId,
    facts: &AnalysisFacts,
) -> ExpressionFacts {
    let mut collected = ExpressionFacts::default();
    for body in graph.bodies().filter(|body| body.origin.source == source) {
        for expression in body.expressions.keys().copied() {
            if let Some(fact) = facts.expression(expression) {
                collected.by_expression.insert(expression, fact.clone());
            }
        }
    }
    collected
}

pub(crate) fn fact_for_range(
    databases: &LanguageServiceDatabases,
    source_id: SourceId,
    range: TextRange,
) -> Option<TypeFact> {
    let graph = databases.hir_db().graph();
    let facts = databases.schema_analysis_facts();
    let expression = graph.expression_containing_span(span_for_range(source_id, range)?)?;
    facts
        .expression(expression)
        .map(widen_public_expression_fact)
}

fn widen_public_expression_fact(fact: &TypeFact) -> TypeFact {
    match fact {
        TypeFact::OptionSome { some } => TypeFact::option((**some).clone()),
        _ => fact.clone(),
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ExpressionFacts {
    by_expression: BTreeMap<HirExprId, TypeFact>,
}

impl ExpressionFacts {
    #[must_use]
    pub(crate) fn get(&self, expression: HirExprId) -> Option<&TypeFact> {
        self.by_expression.get(&expression)
    }

    #[must_use]
    pub(crate) fn fact_for_range(
        &self,
        graph: &ModuleGraph,
        source: SourceId,
        range: TextRange,
    ) -> Option<&TypeFact> {
        let expression = graph.expression_containing_span(span_for_range(source, range)?)?;
        self.get(expression)
    }
}

fn span_for_range(source: SourceId, range: TextRange) -> Option<Span> {
    Some(Span::new(
        source,
        u32::try_from(range.start).ok()?,
        u32::try_from(range.end).ok()?,
    ))
}
