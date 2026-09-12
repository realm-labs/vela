use vela_hir::module_graph::{DeclarationKind, Visibility};
use vela_syntax::{
    SyntaxKind,
    ast::{AstNode, SyntaxPathExpr, SyntaxTypeHint},
};

use crate::{LanguageServiceDatabases, QueryContext, SymbolRef};

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
        let tokens = if let Some(path) = SyntaxPathExpr::cast(node.clone()) {
            path.path_tokens()
        } else if let Some(hint) = SyntaxTypeHint::cast(node) {
            hint.path_tokens()
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
        if !is_type && index + 1 == names.len() && source.is_some() {
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
            if let Some(declaration) = graph.resolve_visible_declaration_path(module, &path, kind) {
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
