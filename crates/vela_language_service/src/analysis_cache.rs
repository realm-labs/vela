use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use vela_analysis::facts::AnalysisFacts;
use vela_analysis::registry::RegistryFacts;
use vela_hir::module_graph::ModuleGraph;

use std::sync::Arc;

/// Whole-workspace [`AnalysisFacts`] memoized for the current workspace
/// generation.
///
/// One keystroke makes the service publish diagnostics and then answer
/// completion, hover, signature help, inlay hint and semantic token requests.
/// Each of those used to infer facts for every body in the workspace from
/// scratch, so the cost of a single edit grew with the number of requests the
/// editor happened to send. The cache builds facts once per generation and
/// hands the same result to every reader.
///
/// Schema-free and schema-backed facts are memoized separately: callers that
/// render script-only answers deliberately resolve without the host schema, and
/// collapsing the two would change what those answers contain.
#[derive(Debug, Default)]
pub(crate) struct AnalysisFactsCache {
    graph_only: OnceLock<Arc<AnalysisFacts>>,
    with_schema: OnceLock<Arc<AnalysisFacts>>,
    builds: AtomicUsize,
}

impl Clone for AnalysisFactsCache {
    fn clone(&self) -> Self {
        Self {
            graph_only: self.graph_only.clone(),
            with_schema: self.with_schema.clone(),
            builds: AtomicUsize::new(self.builds.load(Ordering::Relaxed)),
        }
    }
}

impl AnalysisFactsCache {
    pub(crate) fn graph_only(&self, graph: &ModuleGraph) -> &AnalysisFacts {
        self.graph_only.get_or_init(|| {
            self.builds.fetch_add(1, Ordering::Relaxed);
            Arc::new(AnalysisFacts::from_module_graph(graph))
        })
    }

    pub(crate) fn with_schema(
        &self,
        graph: &ModuleGraph,
        schema: &RegistryFacts,
    ) -> &AnalysisFacts {
        self.with_schema.get_or_init(|| {
            self.builds.fetch_add(1, Ordering::Relaxed);
            Arc::new(AnalysisFacts::from_module_graph_and_schema(graph, schema))
        })
    }

    pub(crate) fn build_count(&self) -> usize {
        self.builds.load(Ordering::Relaxed)
    }

    /// Drops every memoized entry after the module graph changed.
    pub(crate) fn invalidate_graph(&mut self) {
        self.graph_only = OnceLock::new();
        self.with_schema = OnceLock::new();
    }

    /// Drops schema-backed facts after the host schema changed. Schema-free
    /// facts survive because [`AnalysisFacts::from_module_graph`] never reads
    /// the schema.
    pub(crate) fn invalidate_schema(&mut self) {
        self.with_schema = OnceLock::new();
    }
}
