use std::collections::BTreeMap;

use vela_common::SourceId;
use vela_hir::body::HirPathKind;
use vela_hir::module_graph::ModuleGraph;

use crate::hir_path_sites;

#[derive(Debug, Default)]
pub(super) struct PathSiteMaps {
    pub(super) calls: BTreeMap<(usize, usize), Vec<String>>,
    pub(super) expressions: BTreeMap<(usize, usize), Vec<String>>,
    pub(super) patterns: BTreeMap<(usize, usize), Vec<String>>,
}

pub(super) fn collect(graph: &ModuleGraph, source: SourceId) -> PathSiteMaps {
    PathSiteMaps {
        calls: graph
            .paths_in_source_by_kind(source, HirPathKind::Callee)
            .filter_map(hir_path_sites::site)
            .map(|site| {
                (
                    (site.segment_range.start, site.segment_range.end),
                    site.path.to_vec(),
                )
            })
            .collect(),
        expressions: graph
            .paths_in_source(source)
            .filter(|path| hir_path_sites::is_expression_path(path.kind))
            .filter_map(hir_path_sites::site)
            .map(|site| {
                (
                    (site.segment_range.start, site.segment_range.end),
                    site.path.to_vec(),
                )
            })
            .collect(),
        patterns: graph
            .paths_in_source_by_kind(source, HirPathKind::Pattern)
            .filter_map(hir_path_sites::site)
            .map(|site| {
                (
                    (site.segment_range.start, site.segment_range.end),
                    site.path.to_vec(),
                )
            })
            .collect(),
    }
}
