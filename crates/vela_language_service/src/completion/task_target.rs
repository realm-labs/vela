use vela_hir::ids::HirDeclId;
use vela_hir::module_graph::{DeclarationKind, ModuleGraph, Visibility};
use vela_syntax::ast::{AstNode, SyntaxCallExpr, SyntaxExpressionKind};

use crate::{LanguageServiceDatabases, QueryContext};

use super::{CompletionInsertFormat, CompletionItem, CompletionKind};

enum TargetSlot {
    Worker,
    Continuation { worker: Option<HirDeclId> },
}

pub(super) fn adjust_items(
    databases: &LanguageServiceDatabases,
    query: &QueryContext<'_>,
    items: &mut Vec<CompletionItem>,
) {
    let graph = databases.hir_db().graph();
    let Some(slot) = target_slot(graph, query) else {
        return;
    };
    items.retain_mut(|item| {
        // Namespace candidates are navigation steps, not dynamic task values.
        if item.kind() == CompletionKind::Module {
            return true;
        }
        if item.kind() != CompletionKind::Function {
            return false;
        }
        let Some(insertion) = item.insert_text() else {
            return false;
        };
        let path = insertion.strip_suffix("($0)").unwrap_or(insertion);
        let address = query
            .cursor()
            .module_base()
            .map_or_else(|| path.to_owned(), |base| format!("{base}::{path}"));
        let Some(target) = source_target(
            graph,
            query,
            &address.split("::").map(str::to_owned).collect::<Vec<_>>(),
        ) else {
            return false;
        };
        let Some(signature) = graph.function_signature(target) else {
            return false;
        };
        let path_only = match slot {
            TargetSlot::Worker => {
                if !signature.asyncness.is_async() {
                    return false;
                }
                super::callable_path::has_argument_list(
                    query,
                    query
                        .identifier_range()
                        .unwrap_or(query.cursor().replace_range())
                        .end,
                )
            }
            TargetSlot::Continuation { worker } => {
                if signature.asyncness.is_async()
                    || worker.is_some_and(|worker| {
                        vela_analysis::validation::task_continuation_parameter_matches(
                            graph, worker, target,
                        ) != Some(true)
                    })
                {
                    return false;
                }
                true
            }
        };
        if path_only {
            let path = path.to_owned();
            item.insert_text = Some(path.clone());
            item.insert_format = CompletionInsertFormat::PlainText;
            if let Some(edit) = &mut item.metadata.text_edit {
                edit.new_text = path;
                if let Some(range) = query.cursor().identifier_range() {
                    edit.range = range;
                    item.metadata.edit_range = Some(range);
                }
            }
        }
        true
    });
}

fn target_slot(graph: &ModuleGraph, query: &QueryContext<'_>) -> Option<TargetSlot> {
    let offset = query.cursor().replace_range().end;
    // Module-path cursors do not carry the lexical call context. Select the
    // innermost syntax argument list so qualified targets and nested calls use
    // the same operand policy.
    let call = query
        .syntax_parse()?
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxCallExpr::cast)
        .filter(|call| {
            call.l_paren_token()
                .is_some_and(|open| usize::from(open.text_range().end()) <= offset)
                && offset <= usize::from(call.syntax().text_range().end())
        })
        .min_by_key(|call| call.syntax().text_range().len())?;
    let path = call.callee()?.as_path()?.path_segments();
    let [root, operation] = path.as_slice() else {
        return None;
    };
    if root != "task" || !matches!(operation.as_str(), "spawn_scoped" | "spawn_scoped_then") {
        return None;
    }
    let index = call
        .separator_tokens()
        .iter()
        .filter(|token| usize::from(token.text_range().end()) <= offset)
        .count();
    if index > usize::from(operation == "spawn_scoped_then") {
        return None;
    }
    if let Some(expression) = call.arguments().get(index).and_then(|arg| arg.expression()) {
        let direct_path = expression.expression_kind() == SyntaxExpressionKind::Path;
        let worker_callee = index == 0
            && expression
                .as_call()
                .and_then(|call| call.callee())
                .is_some_and(|callee| {
                    callee.expression_kind() == SyntaxExpressionKind::Path
                        && usize::from(callee.syntax().text_range().start())
                            <= query.cursor().replace_range().end
                        && query.cursor().replace_range().end
                            <= usize::from(callee.syntax().text_range().end())
                });
        if !direct_path && !worker_callee {
            return None;
        }
    }
    if index == 0 {
        Some(TargetSlot::Worker)
    } else {
        let worker = call
            .arguments()
            .first()
            .and_then(|argument| argument.expression())
            .and_then(|expression| expression.as_call())
            .and_then(|call| call.callee())
            .and_then(|callee| callee.as_path())
            .and_then(|path| source_target(graph, query, &path.path_segments()));
        Some(TargetSlot::Continuation { worker })
    }
}

fn source_target(
    graph: &ModuleGraph,
    query: &QueryContext<'_>,
    path: &[String],
) -> Option<HirDeclId> {
    if path.is_empty()
        || (path.len() > 1 && matches!(path[0].as_str(), "task" | "service"))
        || query
            .local_bindings_before_cursor()
            .any(|local| local.name == path[0])
    {
        return None;
    }
    let module = graph.module_id(query.module_key()?)?;
    let expanded = graph.expand_import_path(module, path)?;
    let declaration =
        graph.resolve_visible_declaration_path(module, &expanded, DeclarationKind::Function)?;
    (declaration.module == module || declaration.visibility == Visibility::Public)
        .then_some(declaration.id)
}
