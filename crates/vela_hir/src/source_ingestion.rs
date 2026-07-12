//! Production source-set ingestion into Heavy HIR.

use vela_common::Diagnostic;
use vela_syntax::parse::parse_source_with_id;

use crate::ids::ModuleId;
use crate::module_graph::{ModuleGraph, ModuleSource};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirSourceBuildErrorKind {
    Syntax,
    Semantic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSourceBuildError {
    kind: HirSourceBuildErrorKind,
    diagnostics: Vec<Diagnostic>,
}

impl HirSourceBuildError {
    #[must_use]
    pub const fn kind(&self) -> HirSourceBuildErrorKind {
        self.kind
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSourceSet {
    graph: ModuleGraph,
    modules: Box<[ModuleId]>,
}

impl HirSourceSet {
    #[must_use]
    pub const fn graph(&self) -> &ModuleGraph {
        &self.graph
    }

    #[must_use]
    pub fn modules(&self) -> &[ModuleId] {
        &self.modules
    }

    #[must_use]
    pub fn into_parts(self) -> (ModuleGraph, Box<[ModuleId]>) {
        (self.graph, self.modules)
    }
}

pub fn build_source_set(sources: &[ModuleSource]) -> Result<HirSourceSet, HirSourceBuildError> {
    let parsed_sources = sources
        .iter()
        .map(|source| (source, parse_source_with_id(source.id, &source.text)))
        .collect::<Vec<_>>();
    let syntax_diagnostics = parsed_sources
        .iter()
        .flat_map(|(_, parsed)| parsed.diagnostics().iter().cloned())
        .collect::<Vec<_>>();
    if !syntax_diagnostics.is_empty() {
        return Err(HirSourceBuildError {
            kind: HirSourceBuildErrorKind::Syntax,
            diagnostics: syntax_diagnostics,
        });
    }

    let mut graph = ModuleGraph::new();
    let modules = parsed_sources
        .into_iter()
        .map(|(source, parsed)| graph.add_parsed_source(source.clone(), &parsed))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    graph.resolve_imports();
    if !graph.diagnostics().is_empty() {
        return Err(HirSourceBuildError {
            kind: HirSourceBuildErrorKind::Semantic,
            diagnostics: graph.diagnostics().to_vec(),
        });
    }

    Ok(HirSourceSet { graph, modules })
}

#[cfg(test)]
mod tests {
    use vela_common::SourceId;

    use super::*;
    use crate::module_graph::ModulePath;

    fn source(id: u32, path: &[&str], text: &str) -> ModuleSource {
        ModuleSource::new(
            SourceId::new(id),
            ModulePath::new(path.iter().copied()),
            text,
        )
    }

    #[test]
    fn source_set_preserves_input_order_and_empty_root_path() {
        let built = build_source_set(&[
            source(1, &[], "fn main() { return 1; }"),
            source(2, &["game", "reward"], "pub fn grant() { return 2; }"),
        ])
        .expect("source set");

        assert_eq!(built.modules().len(), 2);
        assert_eq!(
            built.graph().module_path(built.modules()[0]),
            Some(&ModulePath::new(Vec::<String>::new()))
        );
        assert_eq!(
            built.graph().module_path(built.modules()[1]),
            Some(&ModulePath::new(["game", "reward"]))
        );
    }

    #[test]
    fn source_set_aggregates_syntax_diagnostics_before_semantics() {
        let error = build_source_set(&[
            source(7, &["game", "one"], "fn one( {"),
            source(8, &["game", "two"], "fn two( {"),
        ])
        .expect_err("syntax errors");

        assert_eq!(error.kind(), HirSourceBuildErrorKind::Syntax);
        assert!(!error.diagnostics().is_empty());
        let sources = error
            .diagnostics()
            .iter()
            .filter_map(|diagnostic| diagnostic.span.map(|span| span.source))
            .collect::<Vec<_>>();
        let first_second_source = sources
            .iter()
            .position(|source| *source == SourceId::new(8))
            .expect("second source diagnostic");
        assert!(
            sources[..first_second_source]
                .iter()
                .all(|source| *source == SourceId::new(7))
        );
    }

    #[test]
    fn duplicate_paths_are_semantic_diagnostics() {
        let error = build_source_set(&[
            source(1, &["game", "reward"], "pub fn one() {}"),
            source(2, &["game", "reward"], "pub fn two() {}"),
        ])
        .expect_err("duplicate module");

        assert_eq!(error.kind(), HirSourceBuildErrorKind::Semantic);
        assert_eq!(
            error.diagnostics()[0].code.as_deref(),
            Some("hir::duplicate_module")
        );
    }

    #[test]
    fn unresolved_imports_are_semantic_diagnostics() {
        let error = build_source_set(&[source(
            1,
            &["game", "main"],
            "use game::missing::run; fn main() {}",
        )])
        .expect_err("unresolved import");

        assert_eq!(error.kind(), HirSourceBuildErrorKind::Semantic);
        assert!(!error.diagnostics().is_empty());
    }
}
