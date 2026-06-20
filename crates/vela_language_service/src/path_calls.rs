use vela_syntax::ast::{AstNode, SyntaxCallExpr, SyntaxPathExpr, SyntaxPattern, SyntaxSourceFile};
use vela_syntax::{Parse as SyntaxParse, SyntaxToken};

use crate::TextRange;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PathCallSite {
    pub(crate) path: Vec<String>,
    pub(crate) segment_range: TextRange,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PathExpressionSite {
    pub(crate) path: Vec<String>,
    pub(crate) segment_range: TextRange,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PatternPathSite {
    pub(crate) path: Vec<String>,
    pub(crate) segment_range: TextRange,
}

pub(crate) fn path_call_sites(parse: &SyntaxParse<SyntaxSourceFile>) -> Vec<PathCallSite> {
    let source = parse.tree();
    source
        .syntax()
        .descendants()
        .filter_map(SyntaxCallExpr::cast)
        .filter_map(|call| {
            let path = call.callee()?.as_path()?;
            let path_segments = path.path_segments();
            let segment_range = last_segment_range(path.path_tokens())?;
            Some(PathCallSite {
                path: path_segments,
                segment_range,
            })
        })
        .collect()
}

pub(crate) fn path_expression_sites(
    parse: &SyntaxParse<SyntaxSourceFile>,
) -> Vec<PathExpressionSite> {
    let source = parse.tree();
    source
        .syntax()
        .descendants()
        .filter_map(SyntaxPathExpr::cast)
        .filter_map(|path| {
            let path_segments = path.path_segments();
            let segment_range = last_segment_range(path.path_tokens())?;
            Some(PathExpressionSite {
                path: path_segments,
                segment_range,
            })
        })
        .collect()
}

pub(crate) fn pattern_path_sites(parse: &SyntaxParse<SyntaxSourceFile>) -> Vec<PatternPathSite> {
    let source = parse.tree();
    source
        .syntax()
        .descendants()
        .filter_map(SyntaxPattern::cast)
        .filter_map(|pattern| {
            let path_segments = pattern.path_segments();
            let segment_range = last_segment_range(pattern.path_tokens())?;
            Some(PatternPathSite {
                path: path_segments,
                segment_range,
            })
        })
        .collect()
}

fn last_segment_range(tokens: Vec<SyntaxToken>) -> Option<TextRange> {
    tokens
        .into_iter()
        .rev()
        .find(|token| token.kind() == vela_syntax::SyntaxKind::Ident)
        .map(|token| {
            let range = token.text_range();
            TextRange::new(range.start().into(), range.end().into())
        })
}

#[cfg(test)]
mod tests {
    use vela_syntax::parse::parse_source;

    use super::*;

    #[test]
    fn path_call_sites_include_path_callees() {
        let source = "\
fn main() {
    grant_reward(1)
    game::reward::grant(2)
    player.level()
}";
        let parsed = parse_source(source);

        let sites = path_call_sites(&parsed);

        let unqualified_start = source.find("grant_reward").expect("plain call");
        let qualified_start =
            source.find("game::reward::grant").expect("qualified call") + "game::reward::".len();
        assert!(sites.contains(&PathCallSite {
            path: vec!["grant_reward".to_owned()],
            segment_range: TextRange::new(
                unqualified_start,
                unqualified_start + "grant_reward".len()
            ),
        }));
        assert!(sites.contains(&PathCallSite {
            path: vec!["game".to_owned(), "reward".to_owned(), "grant".to_owned()],
            segment_range: TextRange::new(qualified_start, qualified_start + "grant".len()),
        }));
        assert!(!sites.iter().any(|site| site.path == ["level"]));
    }

    #[test]
    fn path_expression_sites_include_paths_and_record_constructors() {
        let source = "\
fn main(state: QuestState) {
    let next = QuestState::Active
    let wrapped = QuestState::Active { count: 1 }
}";
        let parsed = parse_source(source);

        let sites = path_expression_sites(&parsed);

        let plain_start =
            source.find("QuestState::Active").expect("path expression") + "QuestState::".len();
        let record_start = source
            .rfind("QuestState::Active")
            .expect("record constructor")
            + "QuestState::".len();
        assert!(sites.contains(&PathExpressionSite {
            path: vec!["QuestState".to_owned(), "Active".to_owned()],
            segment_range: TextRange::new(plain_start, plain_start + "Active".len()),
        }));
        assert!(sites.contains(&PathExpressionSite {
            path: vec!["QuestState".to_owned(), "Active".to_owned()],
            segment_range: TextRange::new(record_start, record_start + "Active".len()),
        }));
    }

    #[test]
    fn pattern_path_sites_include_match_and_for_patterns() {
        let source = "\
fn main(states) {
    for QuestState::Active { count } in states {}
    match state {
        QuestState::Done => 1
        QuestState::Active { count } => count
    }
}";
        let parsed = parse_source(source);

        let sites = pattern_path_sites(&parsed);

        let for_start =
            source.find("QuestState::Active").expect("for pattern") + "QuestState::".len();
        let done_start =
            source.find("QuestState::Done").expect("match path pattern") + "QuestState::".len();
        let match_active_start = source
            .rfind("QuestState::Active")
            .expect("match record pattern")
            + "QuestState::".len();
        assert!(sites.contains(&PatternPathSite {
            path: vec!["QuestState".to_owned(), "Active".to_owned()],
            segment_range: TextRange::new(for_start, for_start + "Active".len()),
        }));
        assert!(sites.contains(&PatternPathSite {
            path: vec!["QuestState".to_owned(), "Done".to_owned()],
            segment_range: TextRange::new(done_start, done_start + "Done".len()),
        }));
        assert!(sites.contains(&PatternPathSite {
            path: vec!["QuestState".to_owned(), "Active".to_owned()],
            segment_range: TextRange::new(match_active_start, match_active_start + "Active".len()),
        }));
    }
}
