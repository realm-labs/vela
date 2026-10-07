//! Independently marked trivia tokens, CST ownership and complete selections.
use super::{Document, Spec, load};
use serde_json::{Value, json};
use vela_syntax::SyntaxNode;

pub(crate) use super::selection::{document, expected, positions};
pub(crate) use super::selection_recovery::assert_parse;

pub(crate) fn helper_queries() -> &'static Value {
    static QUERIES: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    QUERIES.get_or_init(|| load("selection-trivia").oracle["helperQueries"].clone())
}

pub(crate) fn transform(source: &str, crlf: bool, shifted: bool) -> String {
    let source = if shifted {
        // The lexer recognizes a shebang only at byte zero. Shift the remaining
        // source after that entire token, including its newline.
        if source.starts_with("[[file:start]][[shebang:start]]") {
            let end = source.find("[[shebang:end]]").expect("shebang extent");
            let separator = if source[..end].ends_with('\n') {
                ""
            } else {
                "\n"
            };
            source.replacen(
                "[[shebang:end]]",
                &format!("{separator}[[shebang:end]]// shifted 中😀\n/* second 😀 */\n"),
                1,
            )
        } else {
            source.replacen(
                "[[file:start]]",
                "[[file:start]]// shifted 中😀\n/* second 😀 */\n",
                1,
            )
        }
    } else {
        source.to_owned()
    };
    if crlf {
        source.replace('\n', "\r\n")
    } else {
        source
    }
}

pub(crate) fn spec(crlf: bool, shifted: bool) -> Spec {
    let mut spec = load("selection-trivia");
    for source in spec.files.values_mut() {
        *source = transform(source, crlf, shifted);
    }
    for case in spec.oracle["cases"].as_array_mut().expect("cases") {
        case["source"] = json!(transform(
            case["source"].as_str().expect("source"),
            crlf,
            shifted
        ));
    }
    assert_eq!(spec.oracle["cases"].as_array().expect("cases").len(), 63);
    spec
}

pub(crate) fn assert_cst(root: &SyntaxNode, doc: &Document, case: &Value) {
    let mut lexical = doc.clone();
    // Line comments include CR but exclude LF. Markers deliberately never split
    // a CRLF pair, so extend the independently authored byte bound by one.
    for descriptor in case["tokens"].as_array().expect("whole selected token set") {
        let name = descriptor["marker"].as_str().expect("token marker");
        let span = lexical.markers.get_mut(name).expect("literal token extent");
        if descriptor["kind"] == "LineComment"
            && doc.text.as_bytes().get(span.end.byte..span.end.byte + 2) == Some(b"\r\n")
        {
            span.end.byte += 1;
        }
        if descriptor["kind"] == "Whitespace" {
            let bytes = doc.text.as_bytes();
            if bytes.get(span.start.byte..span.start.byte + 2) == Some(b"\r\n")
                && descriptor_before_line_comment(case, doc, span.start.byte)
            {
                span.start.byte += 1;
            } else {
                while span.start.byte > 0 && bytes[span.start.byte - 1].is_ascii_whitespace() {
                    span.start.byte -= 1;
                }
            }
            while bytes
                .get(span.end.byte)
                .is_some_and(u8::is_ascii_whitespace)
            {
                span.end.byte += 1;
            }
        }
        let token = root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|token| {
                let range = token.text_range();
                u32::from(range.start()) as usize == span.start.byte
                    && u32::from(range.end()) as usize == span.end.byte
            })
            .expect("complete independently marked lexical extent");
        assert_eq!(
            format!("{:?}", token.kind()),
            descriptor["kind"].as_str().expect("token kind"),
            "{} {name}",
            case["id"]
        );
        assert_eq!(
            token.kind().is_trivia(),
            descriptor["trivia"]
                .as_bool()
                .expect("explicit trivia policy")
        );
        assert_eq!(token.text(), &doc.text[span.start.byte..span.end.byte]);
    }
    super::selection_recovery::assert_cst(root, &lexical, case);
}

fn descriptor_before_line_comment(case: &Value, doc: &Document, byte: usize) -> bool {
    case["tokens"].as_array().expect("tokens").iter().any(|t| {
        t["kind"] == "LineComment"
            && doc.markers[t["marker"].as_str().expect("marker")].end.byte == byte
    })
}
