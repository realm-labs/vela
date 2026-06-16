mod candidates;
pub mod effects;
pub mod match_exhaustiveness;
pub mod match_patterns;
pub mod member;
mod record_constructors;

use vela_common::Diagnostic;
use vela_hir::{ids::ModuleId, module_graph::ModuleGraph};
use vela_syntax::ast::{Expr, ExprKind, FunctionItem, ImplItem, ItemKind, SourceFile, TraitItem};

use crate::expression::{ExprFactScope, type_fact_from_syntax_hint};
use crate::registry::RegistryFacts;

#[must_use]
pub fn source_diagnostics(source: &SourceFile, facts: &RegistryFacts) -> Vec<Diagnostic> {
    source_diagnostics_with_graph(source, None, facts)
}

#[must_use]
pub fn source_diagnostics_in_module(
    source: &SourceFile,
    graph: &ModuleGraph,
    module: ModuleId,
    facts: &RegistryFacts,
) -> Vec<Diagnostic> {
    source_diagnostics_with_graph(source, Some((graph, module)), facts)
}

fn source_diagnostics_with_graph(
    source: &SourceFile,
    graph_context: Option<(&ModuleGraph, ModuleId)>,
    facts: &RegistryFacts,
) -> Vec<Diagnostic> {
    source
        .items
        .iter()
        .flat_map(|item| match &item.kind {
            ItemKind::Function(function) => function_diagnostics(function, graph_context, facts),
            ItemKind::Trait(item) => trait_diagnostics(item, graph_context, facts),
            ItemKind::Impl(item) => impl_diagnostics(item, graph_context, facts),
            ItemKind::Use(_)
            | ItemKind::Const(_)
            | ItemKind::Global(_)
            | ItemKind::Struct(_)
            | ItemKind::Enum(_) => Vec::new(),
        })
        .collect()
}

fn trait_diagnostics(
    item: &TraitItem,
    graph_context: Option<(&ModuleGraph, ModuleId)>,
    facts: &RegistryFacts,
) -> Vec<Diagnostic> {
    item.methods
        .iter()
        .filter_map(|method| {
            let body = method.default_body.as_ref()?;
            Some(function_body_diagnostics(
                &method.params,
                body,
                graph_context,
                facts,
            ))
        })
        .flatten()
        .collect()
}

fn impl_diagnostics(
    item: &ImplItem,
    graph_context: Option<(&ModuleGraph, ModuleId)>,
    facts: &RegistryFacts,
) -> Vec<Diagnostic> {
    item.methods
        .iter()
        .flat_map(|method| function_diagnostics(&method.function, graph_context, facts))
        .collect()
}

fn function_diagnostics(
    function: &FunctionItem,
    graph_context: Option<(&ModuleGraph, ModuleId)>,
    facts: &RegistryFacts,
) -> Vec<Diagnostic> {
    function_body_diagnostics(&function.params, &function.body, graph_context, facts)
}

fn function_body_diagnostics(
    params: &[vela_syntax::ast::Param],
    body: &vela_syntax::ast::Block,
    graph_context: Option<(&ModuleGraph, ModuleId)>,
    facts: &RegistryFacts,
) -> Vec<Diagnostic> {
    let mut scope = ExprFactScope::new();
    for param in params {
        if let Some(hint) = &param.type_hint {
            scope.insert_path([param.name.clone()], type_fact_from_syntax_hint(hint));
        }
    }
    let expr = Expr {
        kind: ExprKind::Block(body.clone()),
        span: body.span,
    };
    let mut diagnostics = member::member_access_diagnostics(&expr, &scope, facts);
    diagnostics.extend(match_patterns::match_pattern_diagnostics(
        &expr, &scope, facts,
    ));
    diagnostics.extend(match_exhaustiveness::match_exhaustiveness_diagnostics(
        &expr, &scope, facts,
    ));
    if let Some((graph, module)) = graph_context {
        diagnostics.extend(record_constructors::record_constructor_diagnostics(
            &expr, graph, module,
        ));
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use vela_common::SourceId;
    use vela_hir::module_graph::{ModuleGraph, ModulePath};
    use vela_syntax::parser::parse_source;

    use super::*;

    #[test]
    fn source_diagnostics_collect_function_member_errors() {
        let source = parse_source(
            SourceId::new(1),
            "pub fn main(scores: Array<i64>) { return scores.frist() }",
        );

        let diagnostics = source_diagnostics(&source, &RegistryFacts::default());

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_deref() == Some("analysis::unknown_method")
                && diagnostic.span.is_some()
                && diagnostic
                    .candidates
                    .iter()
                    .any(|candidate| candidate.replacement == "first")
                && diagnostic
                    .labels
                    .iter()
                    .any(|label| label.message == "did you mean `first`?")
        }));
    }

    #[test]
    fn source_diagnostics_degrade_unknown_schema_receivers_to_any() {
        let source = parse_source(
            SourceId::new(1),
            "pub fn main(player: Player, scores: Array<i64>) {
                player.level
                scores.frist()
            }",
        );

        let diagnostics = source_diagnostics(&source, &RegistryFacts::default());

        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code.as_deref() == Some("analysis::unknown_method")
            })
        );
        assert!(
            diagnostics.iter().all(|diagnostic| {
                diagnostic.code.as_deref() != Some("analysis::unknown_field")
            })
        );
    }

    #[test]
    fn source_diagnostics_report_missing_required_record_fields() {
        let text = "\
struct Reward {
    amount: i64,
    reason: String = \"quest\",
}

pub fn main() {
    return Reward { reason: \"bonus\" }
}";
        let source = parse_source(SourceId::new(1), text);
        let mut graph = ModuleGraph::new();
        let module = graph.add_parsed_source(
            SourceId::new(1),
            ModulePath::from_qualified("game::main"),
            source.clone(),
        );
        graph.resolve_imports();

        let diagnostics =
            source_diagnostics_in_module(&source, &graph, module, &RegistryFacts::default());

        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code.as_deref() == Some("analysis::missing_constructor_field")
                    && diagnostic
                        .message
                        .contains("missing constructor field `amount`")
            }),
            "{diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diagnostic| {
                !diagnostic
                    .message
                    .contains("missing constructor field `reason`")
            }),
            "{diagnostics:?}"
        );
    }
}
