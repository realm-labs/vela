//! Project static call ownership and named labels at exact syntax spans.
use std::collections::BTreeMap;

use vela_analysis::type_fact::TypeFact;
use vela_common::{SourceId, Span};
use vela_syntax::{
    SyntaxKind, SyntaxToken,
    ast::{AstNode, SyntaxCallExpr},
};

use crate::{
    LanguageServiceDatabases, LineIndex, QueryContext,
    callable_context::{CallableFacts, CallableOrigin, member_callable_facts_for_type},
};

use super::{
    SemanticTokenClassification as C, SemanticTokenModifiers as M, SemanticTokenType as T,
};

pub(super) fn collect(
    db: &LanguageServiceDatabases,
    source: SourceId,
) -> BTreeMap<(usize, usize), C> {
    let mut result = BTreeMap::new();
    let Some(record) = db
        .source_db()
        .records()
        .values()
        .find(|record| record.source_id() == source)
    else {
        return result;
    };
    let Some(parsed) = db.parse_db().syntax_parse(record.document_id()) else {
        return result;
    };
    let lines = LineIndex::new(record.text());
    for call in parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxCallExpr::cast)
    {
        let Some(open) = call.l_paren_token() else {
            continue;
        };
        let Some(query) = QueryContext::from_databases(
            db,
            record.document_id(),
            lines.position(usize::from(open.text_range().end())),
        ) else {
            continue;
        };
        let mut facts = query.call_target_facts(db);
        if facts.is_empty() {
            facts = self_callables(db, &query, source).unwrap_or_default();
        }
        let callable = facts.first();
        if let Some(callable) = callable {
            classify_callee(&call, callable, &mut result);
        }
        for argument in call.arguments() {
            let Some(label) = argument.name_token() else {
                continue;
            };
            let classification = callable
                .filter(|callable| {
                    // Tuple labels retain payload ownership even when argument
                    // validity is diagnosed by a different feature.
                    (callable.supports_named_arguments()
                        || callable.origin() == CallableOrigin::SourceVariant)
                        && callable
                            .params()
                            .iter()
                            .any(|param| param.name() == label.text())
                })
                .map_or_else(
                    || C::new(T::Variable, M::NONE),
                    |callable| {
                        C::new(
                            if callable.origin() == CallableOrigin::SourceVariant {
                                T::Property
                            } else {
                                T::Parameter
                            },
                            modifiers(callable.origin()),
                        )
                    },
                );
            insert(&mut result, &label, classification);
        }
    }
    result
}

fn self_callables(
    db: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    source: SourceId,
) -> Option<Vec<CallableFacts>> {
    let call = query.call_argument_facts()?;
    let range = call.member_receiver()?;
    if query
        .type_fact_for_range(db, range)
        .is_some_and(|fact| !matches!(fact, TypeFact::Unknown))
    {
        return None;
    }
    let graph = db.hir_db().graph();
    let body = graph.body_containing_offset(source, u32::try_from(range.start).ok()?)?;
    let bindings = graph.bindings_for_body(body.id)?;
    let span = Span::new(
        source,
        u32::try_from(range.start).ok()?,
        u32::try_from(range.end).ok()?,
    );
    let resolution = bindings.resolution(graph.expression_containing_span(span)?)?;
    let receiver =
        super::binding_scope::self_receiver(graph, bindings, db.schema_db().facts(), resolution)
            .or_else(|| {
                let vela_hir::binding::BindingResolution::Local(local) = resolution else {
                    return None;
                };
                let binding = bindings.local(*local)?;
                if binding.name != "self"
                    || binding.kind != vela_hir::binding::LocalBindingKind::Parameter
                {
                    return None;
                }
                let owner = graph.declaration(bindings.declaration)?;
                graph
                    .trait_shape(owner.id)?
                    .methods
                    .iter()
                    .any(|method| {
                        method.signature.params.first().is_some_and(|parameter| {
                            parameter.name == "self" && parameter.span == binding.span
                        })
                    })
                    .then(|| {
                        graph
                            .qualified_declaration_name(owner.id)
                            .map(TypeFact::trait_type)
                    })
                    .flatten()
            })?;
    Some(member_callable_facts_for_type(
        db,
        &receiver,
        None,
        call.member_method()?,
        call.args_prefix(),
    ))
}

fn classify_callee(
    call: &SyntaxCallExpr,
    callable: &CallableFacts,
    result: &mut BTreeMap<(usize, usize), C>,
) {
    let Some(callee) = call.callee() else {
        return;
    };
    // Variant paths have a separate enum owner segment and payload-field roles.
    if matches!(
        callable.origin(),
        CallableOrigin::SourceVariant | CallableOrigin::SchemaVariant
    ) {
        return;
    }
    if let Some(path) = callee.as_path() {
        let tokens: Vec<_> = path
            .path_tokens()
            .into_iter()
            .filter(|token| token.kind() == SyntaxKind::Ident)
            .collect();
        for (index, token) in tokens.iter().enumerate() {
            let terminal = index + 1 == tokens.len();
            insert(
                result,
                token,
                if terminal {
                    C::new(T::Function, modifiers(callable.origin()))
                } else {
                    C::new(T::Module, M::NONE)
                },
            );
        }
    } else if let Some(field) = callee.as_field().and_then(|field| field.name_token()) {
        insert(
            result,
            &field,
            C::new(T::Method, modifiers(callable.origin())),
        );
    }
}

fn modifiers(origin: CallableOrigin) -> M {
    match origin {
        CallableOrigin::Source | CallableOrigin::SourceMethod | CallableOrigin::SourceVariant => {
            M::SOURCE
        }
        CallableOrigin::Schema | CallableOrigin::SchemaMethod | CallableOrigin::SchemaVariant => {
            M::HOST.union(M::SCHEMA)
        }
        CallableOrigin::Stdlib | CallableOrigin::StdlibMethod => M::BUILTIN,
    }
}

fn insert(result: &mut BTreeMap<(usize, usize), C>, token: &SyntaxToken, classification: C) {
    result.insert(
        (
            usize::from(token.text_range().start()),
            usize::from(token.text_range().end()),
        ),
        classification,
    );
}
