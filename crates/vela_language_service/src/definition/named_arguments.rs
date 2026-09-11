use vela_analysis::type_fact::TypeFact;
use vela_syntax::{
    SyntaxKind,
    ast::{AstNode, SyntaxCallExpr},
};

use crate::{
    LanguageServiceDatabases, LineIndex, QueryContext, SymbolRef, symbol_target::SymbolTarget,
};

use super::{Definition, text_range_for_span};

#[derive(Default)]
pub(super) struct NamedArgumentNavigation {
    pub(super) definition: Option<Definition>,
    pub(super) type_definition: Option<Definition>,
}

impl LanguageServiceDatabases {
    pub(super) fn named_argument_navigation(
        &self,
        query: &QueryContext<'_>,
        target: &SymbolTarget,
    ) -> Option<NamedArgumentNavigation> {
        let graph = self.hir_db().graph();
        let tree = query.syntax_parse()?.tree();
        for call in tree.syntax().descendants().filter_map(SyntaxCallExpr::cast) {
            if !call.arguments().iter().any(|argument| {
                argument.name_token().is_some_and(|name| {
                    usize::from(name.text_range().start()) == target.range().start
                        && usize::from(name.text_range().end()) == target.range().end
                })
            }) {
                continue;
            }
            // Argument names survive CST recovery even when a missing value
            // prevents HIR from constructing a complete argument record.
            let definition = (|| {
                let expression = call.callee()?;
                let name = expression
                    .as_path()
                    .and_then(|path| path.path_tokens().last().cloned())
                    .filter(|token| token.kind() == SyntaxKind::Ident)
                    .or_else(|| expression.as_field().and_then(|field| field.name_token()))?;
                let offset = usize::from(name.text_range().start());
                let callee = self.definition(
                    query.document_id(),
                    LineIndex::new(query.text()).position(offset),
                )?;
                let parameters = self.source_parameters_for_navigation(&callee)?;
                let (index, parameter) = parameters
                    .params
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.name == target.text())?;
                let inferred = match parameters
                    .declaration
                    .and_then(|id| self.graph_analysis_facts().declaration(id))
                {
                    Some(TypeFact::Function { params, .. }) => params.get(index).cloned(),
                    _ => None,
                };
                let fact = inferred
                    .filter(|fact| !matches!(fact, TypeFact::Unknown))
                    .or_else(|| {
                        parameter.type_hint.as_ref().map(|hint| {
                            crate::callable_context::query_type_fact_from_hint(
                                graph,
                                hint,
                                self.schema_db().facts(),
                            )
                        })
                    });
                let source = self.source_record_for(parameter.span.source)?;
                let range = text_range_for_span(parameter.span)?;
                let symbol = if let Some((owner, variant)) = parameters.variant {
                    crate::symbol_ref::source_variant_field_symbol(
                        graph,
                        owner,
                        variant,
                        &parameter.name,
                    )
                } else {
                    Some(SymbolRef::local_at(
                        &parameter.name,
                        source.document_id().clone(),
                        range,
                    ))
                };
                let definition = self.definition_from_span_with_symbol(parameter.span, symbol);
                Some(NamedArgumentNavigation {
                    definition,
                    type_definition: fact.and_then(|fact| self.type_definition_for_fact(&fact)),
                })
            })();
            return Some(definition.unwrap_or_default());
        }
        None
    }
}
