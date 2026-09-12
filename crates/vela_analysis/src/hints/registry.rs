use vela_hir::{
    ids::ModuleId,
    module_graph::{DeclarationKind, ModuleGraph, Visibility},
    type_hint::HirTypeHint,
};

use crate::{registry::RegistryFacts, type_fact::TypeFact};

/// Resolve every leaf in its declaration's type/import scope. Registry metadata
/// fills only exact, unowned paths; unknown leaves retain their container shape.
pub fn type_fact_from_hint_with_schema(
    graph: &ModuleGraph,
    module: ModuleId,
    hint: &HirTypeHint,
    schema: Option<&RegistryFacts>,
) -> TypeFact {
    if let Some(fact) = super::builtin_type_fact_from_hir_hint(hint, &|arg| {
        type_fact_from_hint_with_schema(graph, module, arg, schema)
    }) {
        return fact;
    }
    if !hint.args.is_empty() {
        return TypeFact::Unknown;
    }
    if hint.path.as_slice() == ["task", "Error"] {
        return TypeFact::record("task::Error");
    }
    let Some(path) = graph.expand_import_path(module, &hint.path) else {
        return TypeFact::Unknown;
    };
    let Some(current) = graph.module_key(module) else {
        return TypeFact::Unknown;
    };
    if let Some(declaration) = [
        DeclarationKind::Struct,
        DeclarationKind::Enum,
        DeclarationKind::Trait,
        DeclarationKind::Function,
        DeclarationKind::Const,
        DeclarationKind::State,
    ]
    .into_iter()
    .find_map(|kind| graph.declaration_by_type_path(&path, current, kind))
    {
        if declaration.module != module && declaration.visibility != Visibility::Public {
            return TypeFact::Unknown;
        }
        return super::declaration_schema_fact(graph, declaration).unwrap_or(TypeFact::Unknown);
    }
    let name = path.join("::");
    schema
        .and_then(|schema| schema.type_fact(&name).or_else(|| schema.trait_fact(&name)))
        .cloned()
        .unwrap_or(TypeFact::Unknown)
}
