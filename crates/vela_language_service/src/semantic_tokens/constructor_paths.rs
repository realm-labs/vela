//! Exact constructor and variant path segments, with module-scoped ownership.
use std::collections::BTreeMap;

use vela_common::{SourceId, Span};
use vela_hir::{
    binding::BindingResolution,
    ids::ModuleId,
    module_graph::{DeclarationKind, Visibility},
};
use vela_syntax::{
    SyntaxKind, SyntaxToken,
    ast::{AstNode, SyntaxPathExpr, SyntaxPattern, SyntaxRecordExpr},
};

use crate::LanguageServiceDatabases;

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
    let Some(module) = db.hir_db().graph().module_id(record.module_key()) else {
        return result;
    };
    for node in parsed.tree().syntax().descendants() {
        let Some((tokens, pattern)) = path_tokens(node) else {
            continue;
        };
        if is_local(db, source, &tokens) {
            continue;
        }
        classify_path(db, module, &tokens, &mut result);
        if pattern
            && tokens.first().is_some_and(|token| {
                !result.contains_key(&(
                    usize::from(token.text_range().start()),
                    usize::from(token.text_range().end()),
                ))
            })
        {
            for token in &tokens {
                result.insert(
                    (
                        usize::from(token.text_range().start()),
                        usize::from(token.text_range().end()),
                    ),
                    C::new(T::UnresolvedReference, M::UNRESOLVED),
                );
            }
        }
    }
    result
}

fn path_tokens(node: vela_syntax::SyntaxNode) -> Option<(Vec<SyntaxToken>, bool)> {
    let is_pattern = SyntaxPattern::can_cast(node.kind());
    let tokens = if let Some(path) = SyntaxPathExpr::cast(node.clone()) {
        path.path_tokens()
    } else if let Some(record) = SyntaxRecordExpr::cast(node.clone()) {
        record.path_tokens()
    } else {
        let pattern = SyntaxPattern::cast(node)?;
        if pattern.is_binding() {
            return None;
        }
        if let Some(record) = pattern.record_pattern() {
            record.path_tokens()
        } else if let Some(tuple) = pattern.tuple_pattern() {
            tuple.path_tokens()
        } else {
            pattern.path_tokens()
        }
    };
    let tokens: Vec<_> = tokens
        .into_iter()
        .filter(|token| token.kind() == SyntaxKind::Ident)
        .collect();

    (!tokens.is_empty()).then_some((tokens, is_pattern))
}

fn is_local(db: &LanguageServiceDatabases, source: SourceId, tokens: &[SyntaxToken]) -> bool {
    let (Some(first), Some(last)) = (tokens.first(), tokens.last()) else {
        return false;
    };
    let graph = db.hir_db().graph();
    let span = Span::new(
        source,
        u32::from(first.text_range().start()),
        u32::from(last.text_range().end()),
    );
    if graph
        .body_containing_offset(source, span.start)
        .and_then(|body| graph.bindings_for_body(body.id))
        .and_then(|bindings| {
            graph
                .expression_containing_span(span)
                .and_then(|expr| bindings.resolution(expr))
        })
        .is_some_and(|resolution| matches!(resolution, BindingResolution::Local(_)))
    {
        return true;
    }

    false
}

fn classify_path(
    db: &LanguageServiceDatabases,
    module: ModuleId,
    tokens: &[SyntaxToken],
    result: &mut BTreeMap<(usize, usize), C>,
) {
    let graph = db.hir_db().graph();
    let path: Vec<_> = tokens.iter().map(|token| token.text().to_owned()).collect();
    let Some(expanded) = graph.expand_import_path(module, &path) else {
        return;
    };
    if let Some(owner) = owner(db, module, &expanded) {
        insert(result, tokens, tokens.len() - 1, owner);
        return;
    }
    let Some((variant, parent)) = expanded.split_last() else {
        return;
    };
    let Some(enum_owner) = owner(db, module, parent).filter(|owner| {
        owner.token_type == T::Enum
            || owner.token_type == T::BuiltinType && owner.modifiers == M::BUILTIN
            || owner.token_type == T::Type
                && db
                    .schema_db()
                    .facts()
                    .type_fact(&parent.join("::"))
                    .is_some_and(|fact| {
                        matches!(fact, vela_analysis::type_fact::TypeFact::Enum { .. })
                    })
    }) else {
        return;
    };
    if tokens.len() < 2 {
        return;
    }
    insert(
        result,
        &tokens[..tokens.len() - 1],
        tokens.len() - 2,
        enum_owner,
    );
    let known = if enum_owner.modifiers == M::BUILTIN {
        vela_analysis::stdlib::stdlib_enum_variants(&parent.join("::"))
            .any(|entry| entry == variant)
    } else if enum_owner.modifiers == M::SOURCE {
        graph
            .resolve_visible_declaration_path(module, parent, DeclarationKind::Enum)
            .and_then(|declaration| graph.enum_shape(declaration.id))
            .is_some_and(|shape| shape.variants.iter().any(|entry| entry.name == *variant))
    } else {
        db.schema_db()
            .facts()
            .variant_fact(&parent.join("::"), variant)
            .is_some()
    };
    let classification = if known {
        C::new(T::EnumMember, enum_owner.modifiers)
    } else {
        C::new(T::UnresolvedReference, M::UNRESOLVED)
    };
    let Some(token) = tokens.last() else {
        return;
    };
    result.insert(
        (
            usize::from(token.text_range().start()),
            usize::from(token.text_range().end()),
        ),
        classification,
    );
}

fn owner(db: &LanguageServiceDatabases, module: ModuleId, path: &[String]) -> Option<C> {
    let graph = db.hir_db().graph();
    let current = graph.module_key(module)?;
    for kind in [
        DeclarationKind::Struct,
        DeclarationKind::Enum,
        DeclarationKind::Trait,
        DeclarationKind::Function,
        DeclarationKind::Const,
        DeclarationKind::State,
    ] {
        if let Some(declaration) = graph.declaration_by_type_path(path, current, kind) {
            return ((declaration.module == module
                || declaration.visibility == Visibility::Public)
                && matches!(kind, DeclarationKind::Struct | DeclarationKind::Enum))
            .then(|| {
                C::new(
                    if kind == DeclarationKind::Enum {
                        T::Enum
                    } else {
                        T::Struct
                    },
                    M::SOURCE,
                )
            });
        }
    }
    if path.len() == 1
        && vela_analysis::stdlib::stdlib_enum_variants(&path[0])
            .next()
            .is_some()
    {
        return Some(C::new(T::BuiltinType, M::BUILTIN));
    }
    db.schema_db()
        .facts()
        .type_fact(&path.join("::"))
        .map(|_| C::new(T::Type, M::HOST.union(M::SCHEMA)))
}

fn insert(
    result: &mut BTreeMap<(usize, usize), C>,
    tokens: &[SyntaxToken],
    terminal: usize,
    owner: C,
) {
    for (index, token) in tokens.iter().enumerate() {
        result.insert(
            (
                usize::from(token.text_range().start()),
                usize::from(token.text_range().end()),
            ),
            if index == terminal {
                owner
            } else {
                C::new(T::Module, M::NONE)
            },
        );
    }
}
