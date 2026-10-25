use std::collections::BTreeMap;

use vela_common::{Diagnostic, SourceId};

use crate::ast::{
    AstNode, SyntaxPathExpr, SyntaxSourceFile, SyntaxTypeHint, SyntaxUseItem, SyntaxUsePath,
};

use super::span_for;

pub(super) fn validate(source: SourceId, tree: &SyntaxSourceFile) -> Vec<Diagnostic> {
    let imports = tree
        .syntax()
        .descendants()
        .filter_map(SyntaxUseItem::cast)
        .filter_map(|item| {
            let path = item.path()?.path_segments();
            let name = item.alias_text().or_else(|| path.last().cloned())?;
            Some((name, path))
        })
        .collect::<BTreeMap<_, _>>();
    tree.syntax()
        .descendants()
        .filter_map(|node| {
            let mut path = if let Some(path) = SyntaxUsePath::cast(node.clone()) {
                path.path_segments()
            } else if let Some(path) = SyntaxPathExpr::cast(node.clone()) {
                path.path_segments()
            } else {
                SyntaxTypeHint::cast(node.clone())?.path_segments()
            };
            // Imports are not an escape hatch for removed compiler-owned paths.
            // Bound the expansion so malformed/cyclic aliases cannot loop.
            for _ in 0..imports.len() {
                if removed_path(&path) {
                    break;
                }
                let Some(prefix) = path.first().and_then(|name| imports.get(name)) else {
                    break;
                };
                let expanded = prefix
                    .iter()
                    .chain(path.iter().skip(1))
                    .cloned()
                    .collect::<Vec<_>>();
                if expanded == path {
                    break;
                }
                path = expanded;
            }
            removed_path(&path).then(|| {
                Diagnostic::error(
                    "script task operations and task error contracts are no longer supported",
                )
                .with_code("syntax::removed_task")
                .with_span(span_for(source, node.text_range()))
            })
        })
        .collect()
}

fn removed_path(path: &[String]) -> bool {
    matches!(path, [root, operation, ..]
        if root == "task" && matches!(operation.as_str(), "spawn_scoped" | "spawn_scoped_then" | "Error"))
}
