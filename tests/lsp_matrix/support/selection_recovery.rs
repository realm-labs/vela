//! Authored recovery ranges and CST ownership, independent of selection queries.
use super::{Document, Spec, load};
use serde_json::{Value, json};
use vela_syntax::SyntaxNode;

pub(crate) use super::selection::{document, expected, positions};
pub(crate) fn helper_queries() -> &'static Value {
    static QUERIES: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    QUERIES.get_or_init(|| load("selection-recovery").oracle["helperQueries"].clone())
}
pub(crate) fn assert_parse<'a>(codes: impl IntoIterator<Item = Option<&'a str>>, policy: &str) {
    let codes = codes.into_iter().collect::<Vec<_>>();
    if let Some(expected) = policy
        .strip_prefix("lexical/")
        .or_else(|| policy.strip_prefix("errors/"))
    {
        let expected = expected
            .split(',')
            .map(Some)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            codes
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            expected
        );
    } else {
        super::selection_brackets::assert_parse(codes, policy);
    }
}

pub(crate) fn spec(crlf: bool, shifted: bool) -> Spec {
    let mut spec = load("selection-recovery");
    let transform = |source: &str| {
        let source = if shifted {
            source.replacen(
                "[[file:start]]",
                "[[file:start]]// shifted 中😀\n/* second 😀 */\n",
                1,
            )
        } else {
            source.to_owned()
        };
        if crlf {
            source.replace('\n', "\r\n")
        } else {
            source
        }
    };
    for source in spec.files.values_mut() {
        *source = transform(source);
    }
    for case in spec.oracle["cases"].as_array_mut().expect("cases") {
        case["source"] = json!(transform(case["source"].as_str().expect("source")));
    }
    assert_eq!(spec.oracle["cases"].as_array().expect("cases").len(), 250);
    spec
}

pub(crate) fn assert_cst(root: &SyntaxNode, doc: &Document, case: &Value) {
    let Some(nodes) = case["nodes"].as_array() else {
        return;
    };
    assert_eq!(root.text().to_string(), doc.text);
    let extent = |node: &SyntaxNode| {
        let range = node.text_range();
        (
            format!("{:?}", node.kind()),
            u32::from(range.start()) as usize,
            u32::from(range.end()) as usize,
        )
    };
    let authored = |descriptor: &Value| {
        let marker = doc.markers[descriptor["marker"].as_str().expect("node marker")];
        (
            descriptor["kind"].as_str().expect("node kind").to_owned(),
            marker.start.byte,
            marker.end.byte,
        )
    };
    let all = root
        .descendants()
        .map(|node| extent(&node))
        .collect::<Vec<_>>();
    for node in nodes {
        let expected = authored(node);
        assert!(
            all.contains(&expected),
            "{} missing authored CST node {expected:?}",
            case["id"]
        );
    }
    for query in case["queries"].as_array().expect("queries") {
        let Some(token) = query["token"].as_str() else {
            continue;
        };
        let marker = doc.markers[token];
        let token = root
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| {
                let range = token.text_range();
                u32::from(range.start()) as usize == marker.start.byte
                    && u32::from(range.end()) as usize == marker.end.byte
            })
            .expect("complete independently marked lexical token");
        let actual = token
            .parent()
            .expect("CST owner")
            .ancestors()
            .map(|node| extent(&node))
            .collect::<Vec<_>>();
        let expected = query["ancestors"]
            .as_array()
            .expect("whole authored CST ancestry")
            .iter()
            .map(authored)
            .collect::<Vec<_>>();
        assert_eq!(
            actual, expected,
            "{} {} exact CST ancestry",
            case["id"], query["position"]
        );
    }
}
