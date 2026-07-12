//! Production source-set ingestion into Heavy HIR.

use vela_common::{Diagnostic, SourceId};
use vela_syntax::parse::parse_source_with_id;

use crate::ids::ModuleId;
use crate::module_graph::{DeclarationKind, ModuleGraph, ModulePath, ModuleSource};

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
    kind: HirSourceSetKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirSourceSetKind {
    SingleSource,
    ModuleGraph,
}

#[derive(Clone, Copy)]
pub struct HirSourceFunction<'source> {
    sources: &'source HirSourceSet,
    declaration: crate::ids::HirDeclId,
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
    pub const fn kind(&self) -> HirSourceSetKind {
        self.kind
    }

    #[must_use]
    pub fn function(&self, module_path: &ModulePath, name: &str) -> Option<HirSourceFunction<'_>> {
        let module = self.graph.module_id(module_path)?;
        let declaration = self.graph.module(module)?.get(name)?;
        let metadata = self.graph.declaration(declaration)?;
        (metadata.kind == DeclarationKind::Function
            && self.graph.function_body(declaration).is_some())
        .then_some(HirSourceFunction {
            sources: self,
            declaration,
        })
    }
}

impl<'source> HirSourceFunction<'source> {
    #[must_use]
    pub const fn sources(self) -> &'source HirSourceSet {
        self.sources
    }

    #[must_use]
    pub const fn declaration(self) -> crate::ids::HirDeclId {
        self.declaration
    }
}

pub fn build_single_source(
    source: SourceId,
    text: impl Into<String>,
) -> Result<HirSourceSet, HirSourceBuildError> {
    build_source_set(
        &[ModuleSource::new(source, ModulePath::root(), text)],
        HirSourceSetKind::SingleSource,
    )
}

pub fn build_module_source_set(
    sources: &[ModuleSource],
) -> Result<HirSourceSet, HirSourceBuildError> {
    build_source_set(sources, HirSourceSetKind::ModuleGraph)
}

fn build_source_set(
    sources: &[ModuleSource],
    kind: HirSourceSetKind,
) -> Result<HirSourceSet, HirSourceBuildError> {
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

    Ok(HirSourceSet {
        graph,
        modules,
        kind,
    })
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
    fn source_set_preserves_mode_input_order_and_empty_root_path() {
        let single = build_single_source(SourceId::new(1), "fn main() { return 1; }")
            .expect("single source set");
        assert_eq!(single.kind(), HirSourceSetKind::SingleSource);
        assert_eq!(
            single.graph().module_path(single.modules()[0]),
            Some(&ModulePath::root())
        );

        let built = build_module_source_set(&[
            source(1, &[], "fn main() { return 1; }"),
            source(2, &["game", "reward"], "pub fn grant() { return 2; }"),
        ])
        .expect("source set");

        assert_eq!(built.kind(), HirSourceSetKind::ModuleGraph);
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
        let sources = [
            source(7, &["game", "one"], "fn one( {"),
            source(8, &["game", "two"], "fn two( {"),
        ];
        let expected = sources
            .iter()
            .flat_map(|source| {
                parse_source_with_id(source.id, &source.text)
                    .diagnostics()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let error = build_module_source_set(&sources).expect_err("syntax errors");

        assert_eq!(error.kind(), HirSourceBuildErrorKind::Syntax);
        assert_eq!(error.diagnostics(), expected);
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
        let error = build_module_source_set(&[
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
        let sources = [source(
            1,
            &["game", "main"],
            "use game::missing::run; fn main() {}",
        )];
        let mut expected_graph = ModuleGraph::new();
        for source in &sources {
            expected_graph.add_source(source.clone());
        }
        expected_graph.resolve_imports();
        let expected = expected_graph.diagnostics().to_vec();
        let error = build_module_source_set(&sources).expect_err("unresolved import");

        assert_eq!(error.kind(), HirSourceBuildErrorKind::Semantic);
        assert_eq!(error.diagnostics(), expected);
    }
}
