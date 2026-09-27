use vela_analysis::type_fact::TypeFact;

use super::Hover;
use crate::{DiagnosticRange, LanguageServiceDatabases, symbol_target::SymbolTarget};

pub(super) fn has_source_owner(
    graph: &vela_hir::module_graph::ModuleGraph,
    fact: &TypeFact,
) -> bool {
    use vela_hir::module_graph::DeclarationKind;
    let (name, kind) = match fact {
        TypeFact::Record { name } => (name, DeclarationKind::Struct),
        TypeFact::Enum { name, .. } => (name, DeclarationKind::Enum),
        TypeFact::Trait { name } => (name, DeclarationKind::Trait),
        _ => return false,
    };
    // Collection iteration can retain a canonical fact without a source-origin
    // set. The exact source owner still owns a missing member; short names and
    // host facts must not acquire source identity through this check.
    graph.declarations().any(|declaration| {
        declaration.kind == kind && super::qualified_declaration_label(graph, declaration) == *name
    })
}

pub(super) fn hover(
    db: &LanguageServiceDatabases,
    target: &SymbolTarget,
    range: DiagnosticRange,
) -> Option<Hover> {
    if target.has_incomplete_member_origins() {
        return None;
    }
    let fact = target.member_receiver_fact()?;
    let candidates = target.possible_member_targets();
    let graph = db.hir_db().graph();
    // A script-origin set cannot stand in for a registry, primitive or dynamic
    // alternative in the receiver's complete type. No owner may be guessed.
    if let TypeFact::Union(values) = fact
        && candidates
            .iter()
            .any(|candidate| candidate.member_receiver_declaration().is_some())
        && !values.iter().all(|value| {
            let name = match value {
                TypeFact::Record { name }
                | TypeFact::Enum { name, .. }
                | TypeFact::Trait { name } => name,
                _ => return false,
            };
            candidates.iter().any(|candidate| {
                candidate
                    .member_receiver_declaration()
                    .and_then(|id| graph.declaration(id))
                    .is_some_and(|declaration| {
                        super::qualified_declaration_label(graph, declaration) == *name
                    })
            })
        })
    {
        return None;
    }
    if candidates.len() == 1 {
        return db.member_hover(fact, &candidates[0], range);
    }
    let facts = db.graph_analysis_facts();
    let mut results = candidates.iter().map(|candidate| {
        let receiver = facts.declaration(candidate.member_receiver_declaration()?)?;
        db.member_hover(receiver, candidate, range)
    });
    let first = results.next()??;
    results
        .all(|result| result.as_ref() == Some(&first))
        .then_some(first)
}
