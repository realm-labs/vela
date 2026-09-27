use vela_hir::module_graph::{DeclarationKind, Visibility};
use vela_syntax::{
    SyntaxKind,
    ast::{AstNode, SyntaxPathExpr, SyntaxPattern, SyntaxPatternKind, SyntaxTypeHint},
};

use crate::{
    LanguageServiceDatabases, QueryContext, SymbolRef,
    query_context::binding_resolution_for_source_range,
};

/// A qualified path owns even an unresolved result, preventing fallback to an
/// unrelated unqualified schema or builtin name. Terminal bindings retain their
/// owner; a module prefix must not inherit the binding of the whole path.
pub(super) fn path_symbol_ref(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    source: Option<&SymbolRef>,
) -> Option<Option<SymbolRef>> {
    let range = query.identifier_range()?;
    let graph = databases.hir_db().graph();
    let key = query.module_key()?;
    let module = graph.module_id(key)?;
    for node in query.syntax_parse()?.tree().syntax().descendants() {
        let is_type = SyntaxTypeHint::can_cast(node.kind());
        let is_pattern = SyntaxPattern::can_cast(node.kind());
        let tokens = if let Some(path) = SyntaxPathExpr::cast(node.clone()) {
            path.path_tokens()
        } else if let Some(hint) = SyntaxTypeHint::cast(node.clone()) {
            hint.path_tokens()
        } else if let Some(pattern) = SyntaxPattern::cast(node)
            && matches!(
                pattern.pattern_kind(),
                Some(
                    SyntaxPatternKind::Path
                        | SyntaxPatternKind::TupleVariant
                        | SyntaxPatternKind::RecordVariant
                )
            )
        {
            pattern.path_tokens()
        } else {
            continue;
        };
        let names = tokens
            .into_iter()
            .filter(|token| matches!(token.kind(), SyntaxKind::Ident | SyntaxKind::SelfKw))
            .collect::<Vec<_>>();
        let Some(index) = names.iter().position(|token| {
            usize::from(token.text_range().start()) == range.start
                && usize::from(token.text_range().end()) == range.end
        }) else {
            continue;
        };
        // HIR may resolve an invalid Enum::Variant path to the enum owner.
        // Validate that selected terminal segment before retaining its binding.
        let enum_owner = source.is_some_and(|symbol| {
            query
                .bindings()
                .and_then(|bindings| binding_resolution_for_source_range(graph, bindings, range))
                .and_then(|resolution| match resolution {
                    vela_hir::binding::BindingResolution::Declaration(id) => graph.declaration(*id),
                    _ => None,
                })
                .is_some_and(|declaration| {
                    declaration.kind == DeclarationKind::Enum
                        && crate::symbol_ref::source_symbol_for_declaration(graph, declaration)
                            == *symbol
                })
        });
        if !is_type && !is_pattern && index + 1 == names.len() && source.is_some() && !enum_owner {
            return Some(source.cloned());
        }
        let path = names[..=index]
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>();
        let Some(path) = (if is_type {
            graph.expand_import_path(module, &path)
        } else {
            query.expand_import_path(&path)
        }) else {
            return Some(None);
        };
        if let Some(module_key) = graph.resolve_module_path(key, &path)
            && graph.module_id(&module_key).is_some()
        {
            return Some(Some(crate::symbol_ref::source_module_symbol(&module_key)));
        }
        for kind in [
            DeclarationKind::Function,
            DeclarationKind::Const,
            DeclarationKind::State,
            DeclarationKind::Struct,
            DeclarationKind::Enum,
            DeclarationKind::Trait,
        ] {
            if let Some(declaration) = graph.declaration_by_type_path(&path, key, kind) {
                return Some(
                    ((!is_type
                        || matches!(
                            kind,
                            DeclarationKind::Struct
                                | DeclarationKind::Enum
                                | DeclarationKind::Trait
                        ))
                        && (declaration.module == module
                            || declaration.visibility == Visibility::Public))
                        .then(|| {
                            crate::symbol_ref::source_symbol_for_declaration(graph, declaration)
                        }),
                );
            }
        }
        if let Some((variant, owner)) = path.split_last()
            && let Some(declaration) =
                graph.declaration_by_type_path(owner, key, DeclarationKind::Enum)
        {
            return Some(
                (!is_type
                    && (declaration.module == module
                        || declaration.visibility == Visibility::Public))
                    .then(|| {
                        graph
                            .enum_shape(declaration.id)?
                            .variants
                            .iter()
                            .find(|entry| entry.name == *variant)
                            .and_then(|entry| {
                                crate::symbol_ref::source_enum_variant_symbol(
                                    graph,
                                    declaration.id,
                                    &entry.name,
                                )
                            })
                    })
                    .flatten(),
            );
        }
        let qualified = path.join("::");
        if let Some(symbol) = super::schema_symbol_ref(databases.schema_db().facts(), &qualified) {
            return Some(Some(symbol));
        }
        if let Some(symbol) = super::stdlib_function_symbol_ref(&qualified) {
            return Some(Some(symbol));
        }
        if vela_analysis::stdlib::stdlib_function_completion_facts()
            .iter()
            .any(|function| function.name.starts_with(&format!("{qualified}::")))
        {
            return Some(Some(crate::symbol_ref::builtin_symbol(qualified)));
        }
        return (names.len() > 1).then_some(None);
    }
    None
}
