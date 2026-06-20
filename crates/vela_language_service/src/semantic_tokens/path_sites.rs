use std::collections::BTreeMap;

use vela_syntax::Parse as SyntaxParse;
use vela_syntax::ast::SyntaxSourceFile;

use crate::path_calls;

#[derive(Debug, Default)]
pub(super) struct PathSiteMaps {
    pub(super) calls: BTreeMap<(usize, usize), Vec<String>>,
    pub(super) expressions: BTreeMap<(usize, usize), Vec<String>>,
    pub(super) patterns: BTreeMap<(usize, usize), Vec<String>>,
}

pub(super) fn collect(parsed: &SyntaxParse<SyntaxSourceFile>) -> PathSiteMaps {
    PathSiteMaps {
        calls: path_calls::path_call_sites(parsed)
            .into_iter()
            .map(|site| {
                (
                    (site.segment_range.start, site.segment_range.end),
                    site.path,
                )
            })
            .collect(),
        expressions: path_calls::path_expression_sites(parsed)
            .into_iter()
            .map(|site| {
                (
                    (site.segment_range.start, site.segment_range.end),
                    site.path,
                )
            })
            .collect(),
        patterns: path_calls::pattern_path_sites(parsed)
            .into_iter()
            .map(|site| {
                (
                    (site.segment_range.start, site.segment_range.end),
                    site.path,
                )
            })
            .collect(),
    }
}
