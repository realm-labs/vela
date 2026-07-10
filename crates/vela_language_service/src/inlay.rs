use std::collections::BTreeMap;

use vela_analysis::{
    facts::AnalysisFacts, stdlib::stdlib_method_fact_with_lambda_arity, type_fact::TypeFact,
};
use vela_common::SourceId;
use vela_hir::{
    binding::LocalBindingKind,
    body::{HirBody, HirCall, HirExprKind, HirPathKind, HirPathOwner},
};

use crate::callable_context::{
    CallableParameterFacts, callable_facts, member_callable_facts_for_type,
};
use crate::symbol_ref::{builtin_member_symbol, schema_member_symbol, source_child_symbol};
use crate::{
    DiagnosticRange, DisplayParts, DocumentId, LanguageServiceDatabases, LineIndex, Position,
    SymbolRef, TextRange,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum InlayHintKind {
    Type,
    Parameter,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InlayHint {
    position: Position,
    label: DisplayParts,
    kind: InlayHintKind,
    symbol: Option<SymbolRef>,
}

#[derive(Clone, Copy)]
struct DiagnosticRangeOffsets {
    start: usize,
    end: usize,
}

impl DiagnosticRangeOffsets {
    const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    const fn contains(self, offset: usize) -> bool {
        self.start <= offset && offset <= self.end
    }
}

#[derive(Clone, Copy)]
struct ParameterHintContext<'a> {
    source_id: SourceId,
    source_text: &'a str,
    line_index: &'a LineIndex,
    range: DiagnosticRangeOffsets,
}

impl<'a> ParameterHintContext<'a> {
    const fn new(
        source_id: SourceId,
        source_text: &'a str,
        line_index: &'a LineIndex,
        range: DiagnosticRangeOffsets,
    ) -> Self {
        Self {
            source_id,
            source_text,
            line_index,
            range,
        }
    }
}

impl InlayHint {
    #[must_use]
    pub const fn position(&self) -> Position {
        self.position
    }

    #[must_use]
    pub fn label(&self) -> String {
        self.label.render()
    }

    #[must_use]
    pub fn label_parts(&self) -> &DisplayParts {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> InlayHintKind {
        self.kind
    }

    #[must_use]
    pub const fn symbol(&self) -> Option<&SymbolRef> {
        self.symbol.as_ref()
    }
}

impl LanguageServiceDatabases {
    #[must_use]
    pub fn inlay_hints(&self, document_id: &DocumentId, range: DiagnosticRange) -> Vec<InlayHint> {
        let Some(source) = self.source_db().records().get(document_id) else {
            return Vec::new();
        };
        let line_index = LineIndex::new(source.text());
        let range_start = line_index.offset(range.start());
        let range_end = line_index.offset(range.end());
        let range_offsets = DiagnosticRangeOffsets::new(range_start, range_end);
        let parameter_context = ParameterHintContext::new(
            source.source_id(),
            source.text(),
            &line_index,
            range_offsets,
        );
        let mut hints = Vec::new();

        self.collect_hir_parameter_hints(parameter_context, &mut hints);

        self.collect_hir_type_hints(
            document_id,
            source.source_id(),
            &line_index,
            range_offsets,
            &mut hints,
        );

        hints.sort_by_key(|hint| (hint.position.line, hint.position.character));
        hints
    }

    fn collect_hir_parameter_hints(
        &self,
        context: ParameterHintContext<'_>,
        hints: &mut Vec<InlayHint>,
    ) {
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph_and_schema(graph, self.schema_db().facts());
        for body in graph
            .bodies()
            .filter(|body| body.origin.source == context.source_id)
        {
            for (_, call) in body.calls() {
                let args_prefix = hir_args_prefix(body, call, context.source_text);
                let callable = if let Some(field) = body.field(call.callee) {
                    let Some(receiver) = facts.expression(field.receiver) else {
                        continue;
                    };
                    member_callable_facts_for_type(self, receiver, &field.name, &args_prefix)
                        .into_iter()
                        .next()
                } else {
                    body.paths
                        .iter()
                        .find(|path| {
                            path.owner == HirPathOwner::Expression(call.callee)
                                && path.kind == HirPathKind::Callee
                        })
                        .and_then(|path| {
                            callable_facts(self, &path.path.join("::"))
                                .into_iter()
                                .next()
                        })
                };
                let Some(callable) = callable else {
                    continue;
                };
                for (index, argument) in call.arguments.iter().enumerate() {
                    if argument.name.is_some() {
                        continue;
                    }
                    let Some(value) = argument.value else {
                        continue;
                    };
                    if body
                        .expression(value)
                        .is_some_and(|expr| matches!(expr.kind, HirExprKind::Lambda { .. }))
                    {
                        continue;
                    }
                    let offset = argument.origin.span.start as usize;
                    if !context.range.contains(offset) {
                        continue;
                    }
                    let Some(parameter) = callable.params().get(index) else {
                        continue;
                    };
                    let Some(label) = parameter_hint_label(parameter) else {
                        continue;
                    };
                    hints.push(InlayHint {
                        position: context.line_index.position(offset),
                        label,
                        kind: InlayHintKind::Parameter,
                        symbol: Some(parameter_symbol(callable.symbol(), parameter.name())),
                    });
                }
            }
        }
    }

    fn collect_hir_type_hints(
        &self,
        document_id: &DocumentId,
        source: SourceId,
        line_index: &LineIndex,
        range: DiagnosticRangeOffsets,
        hints: &mut Vec<InlayHint>,
    ) {
        let graph = self.hir_db().graph();
        let facts = AnalysisFacts::from_module_graph_and_schema(graph, self.schema_db().facts());
        let mut contextual_locals = BTreeMap::new();
        for body in graph.bodies().filter(|body| body.origin.source == source) {
            for (_, call) in body.calls() {
                let Some(field) = body.field(call.callee) else {
                    continue;
                };
                let Some(receiver) = facts.expression(field.receiver) else {
                    continue;
                };
                for argument in &call.arguments {
                    let Some(value) = argument.value else {
                        continue;
                    };
                    let Some(HirExprKind::Lambda { body: lambda_body }) =
                        body.expression(value).map(|expr| &expr.kind)
                    else {
                        continue;
                    };
                    let Some(lambda_body) = graph.body(*lambda_body) else {
                        continue;
                    };
                    let Some(params) = stdlib_method_fact_with_lambda_arity(
                        receiver,
                        &field.name,
                        None,
                        Some(lambda_body.params.len()),
                    )
                    .and_then(|fact| fact.lambda.map(|lambda| lambda.params)) else {
                        continue;
                    };
                    contextual_locals.extend(
                        lambda_body
                            .params
                            .iter()
                            .zip(params)
                            .map(|(param, fact)| (param.local, fact)),
                    );
                }
            }
        }

        for body in graph.bodies().filter(|body| body.origin.source == source) {
            for local in &body.locals {
                let Some(binding) = graph.local_binding(*local) else {
                    continue;
                };
                if binding.kind == LocalBindingKind::Parameter || binding.type_hint.is_some() {
                    continue;
                }
                let fact = contextual_locals.get(local).or_else(|| facts.local(*local));
                let Some(fact) = fact else {
                    continue;
                };
                let Some(label) = type_hint_label(fact) else {
                    continue;
                };
                let offset = binding.span.end as usize;
                if !range.contains(offset) {
                    continue;
                }
                hints.push(InlayHint {
                    position: line_index.position(offset),
                    label,
                    kind: InlayHintKind::Type,
                    symbol: Some(SymbolRef::local_at(
                        binding.name.clone(),
                        document_id.clone(),
                        TextRange::new(binding.span.start as usize, offset),
                    )),
                });
            }
            for (expression, field) in body.fields() {
                if body.calls().any(|(_, call)| call.callee == expression) {
                    continue;
                }
                let Some(TypeFact::Host { name: owner }) = facts.expression(field.receiver) else {
                    continue;
                };
                let Some(fact) = facts.expression(expression) else {
                    continue;
                };
                let Some(label) = type_hint_label(fact) else {
                    continue;
                };
                let offset = field.member_origin.span.end as usize;
                if !range.contains(offset) {
                    continue;
                }
                hints.push(InlayHint {
                    position: line_index.position(offset),
                    label,
                    kind: InlayHintKind::Type,
                    symbol: Some(schema_member_symbol(owner, &field.name)),
                });
            }
        }
    }
}

fn hir_args_prefix(body: &HirBody, call: &HirCall, source_text: &str) -> String {
    let Some(callee) = body.expression(call.callee) else {
        return String::new();
    };
    let end = call
        .arguments
        .last()
        .map_or(callee.origin.span.end, |argument| argument.origin.span.end);
    source_text
        .get(callee.origin.span.end as usize..end as usize)
        .unwrap_or_default()
        .to_owned()
}

fn parameter_hint_label(parameter: &CallableParameterFacts) -> Option<DisplayParts> {
    if !is_stable_type_fact(parameter.type_fact()) {
        return None;
    }
    let name = parameter.name();
    (!name.is_empty()).then(|| DisplayParts::parameter_hint(name))
}

fn parameter_symbol(callable: &SymbolRef, parameter: &str) -> SymbolRef {
    match callable {
        SymbolRef::Source(symbol) => source_child_symbol(symbol, parameter),
        SymbolRef::Schema(symbol) => schema_member_symbol(symbol, parameter),
        SymbolRef::Builtin(symbol) => builtin_member_symbol(symbol, parameter),
        SymbolRef::Local(symbol) => SymbolRef::local(format!("{}.{}", symbol.name(), parameter)),
    }
}

fn type_hint_label(fact: &TypeFact) -> Option<DisplayParts> {
    is_stable_type_fact(fact).then(|| {
        let type_name = fact.display_name();
        DisplayParts::type_annotation(&type_name)
    })
}

fn is_stable_type_fact(fact: &TypeFact) -> bool {
    match fact {
        TypeFact::Unknown | TypeFact::Any | TypeFact::Never => false,
        TypeFact::Array { element }
        | TypeFact::Set { element }
        | TypeFact::Iterator { item: element }
        | TypeFact::Option { some: element }
        | TypeFact::OptionSome { some: element }
        | TypeFact::ResultOk { ok: element }
        | TypeFact::ResultErr { err: element } => is_stable_type_fact(element),
        TypeFact::Map { key, value }
        | TypeFact::Result {
            ok: key,
            err: value,
        } => is_stable_type_fact(key) && is_stable_type_fact(value),
        TypeFact::Function { params, returns } => {
            params.iter().all(is_stable_type_fact) && is_stable_type_fact(returns)
        }
        TypeFact::Tuple { elements } => elements.iter().all(is_stable_type_fact),
        TypeFact::Union(facts) => {
            !facts.is_empty()
                && facts.iter().all(is_stable_type_fact)
                && facts.iter().any(|fact| !matches!(fact, TypeFact::Never))
        }
        TypeFact::Primitive(_)
        | TypeFact::Range
        | TypeFact::Closure
        | TypeFact::OptionNone
        | TypeFact::Record { .. }
        | TypeFact::LogicalRecord(_)
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Trait { .. }
        | TypeFact::Module { .. } => true,
    }
}

#[cfg(test)]
mod suppression_tests;
#[cfg(test)]
mod tests;
