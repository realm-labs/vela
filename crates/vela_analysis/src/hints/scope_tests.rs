use super::*;
use vela_common::{SourceId, Span};
use vela_hir::module_graph::ModuleSource;
use vela_package::{ModulePath, PackageId};

#[test]
fn module_type_hints_preserve_local_import_and_visibility_identity() {
    let mut graph = ModuleGraph::new();
    for (index, module, text) in [
        (
            1,
            "alpha",
            "pub struct Cell {} pub struct OnlyForeign {} struct Hidden {}",
        ),
        (
            2,
            "beta",
            "use alpha::Cell as Imported; struct Cell {} fn main() {}",
        ),
    ] {
        graph.add_source(ModuleSource::new(
            SourceId::new(index),
            PackageId::anonymous(),
            ModulePath::from_qualified(module),
            text,
        ));
    }
    graph.resolve_imports();
    assert!(graph.diagnostics().is_empty());
    let module = graph
        .declarations()
        .find(|declaration| declaration.name == "main")
        .expect("requesting module")
        .module;
    for (path, expected) in [
        (vec!["Cell"], TypeFact::record("beta::Cell")),
        (vec!["Imported"], TypeFact::record("alpha::Cell")),
        (vec!["alpha", "Cell"], TypeFact::record("alpha::Cell")),
        (vec!["alpha", "Hidden"], TypeFact::Unknown),
        (vec!["OnlyForeign"], TypeFact::Unknown),
        (vec!["i64"], TypeFact::I64),
    ] {
        let hint = HirTypeHint {
            path: path.iter().map(|name| (*name).to_owned()).collect(),
            args: Vec::new(),
            span: Span::new(SourceId::new(2), 0, 0),
        };
        assert_eq!(
            type_fact_from_hint_in_module(&graph, module, &hint),
            expected,
            "{path:?}"
        );
        let nested = HirTypeHint {
            path: vec!["Option".into()],
            args: vec![hint],
            span: Span::new(SourceId::new(2), 0, 0),
        };
        assert_eq!(
            type_fact_from_hint_in_module(&graph, module, &nested),
            TypeFact::option(expected),
            "nested {path:?}"
        );
    }
}
