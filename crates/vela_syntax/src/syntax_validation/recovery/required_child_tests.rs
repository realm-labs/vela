use crate::{SyntaxKind, parse::parse_source};

#[test]
fn explicit_annotations_and_import_separators_retain_required_child_errors_after_recovery() {
    for (damaged, repaired, node, kind, highlight, message) in [
        (
            "fn run(value:) {}",
            "fn run(value: i64) {}",
            "value:",
            SyntaxKind::Param,
            "value:",
            "expected type annotation",
        ),
        (
            "struct Cell { value: }",
            "struct Cell { value: i64 }",
            "value:",
            SyntaxKind::StructField,
            "value:",
            "expected type annotation",
        ),
        (
            "enum Choice { One(value:) }",
            "enum Choice { One(value: i64) }",
            "value:",
            SyntaxKind::Param,
            "value:",
            "expected type annotation",
        ),
        (
            "enum Choice { Record { value: } }",
            "enum Choice { Record { value: i64 } }",
            "value:",
            SyntaxKind::StructField,
            "value:",
            "expected type annotation",
        ),
        (
            "trait Task { fn run(value:); }",
            "trait Task { fn run(value: i64); }",
            "value:",
            SyntaxKind::Param,
            "value:",
            "expected type annotation",
        ),
        (
            "impl Cell { fn run(value:) {} }",
            "impl Cell { fn run(value: i64) {} }",
            "value:",
            SyntaxKind::Param,
            "value:",
            "expected type annotation",
        ),
        (
            "fn run() { let f = |value:| value; }",
            "fn run() { let f = |value: i64| value; }",
            "value:",
            SyntaxKind::Param,
            "value:",
            "expected type annotation",
        ),
        (
            "use module::;",
            "use module::value;",
            "module::",
            SyntaxKind::UsePath,
            "::",
            "expected import path segment",
        ),
        (
            "use module:: as kept;",
            "use module::value as kept;",
            "module::",
            SyntaxKind::UsePath,
            "::",
            "expected import path segment",
        ),
    ] {
        for newline in ["\n", "\r\n"] {
            for shifted in [false, true] {
                for fragment in [damaged, repaired, damaged] {
                    let prefix = if shifted {
                        "// shifted 中😀\n/* second 😀 */\n"
                    } else {
                        ""
                    };
                    let source = format!("{prefix}/*中😀*/ fn before() {{}}\n{fragment}\n")
                        .replace('\n', newline);
                    let parsed = parse_source(&source);
                    assert_eq!(parsed.syntax_node().to_string(), source);
                    if fragment == repaired {
                        assert!(
                            parsed.diagnostics().is_empty(),
                            "repaired {fragment}: {:?}",
                            parsed.diagnostics()
                        );
                        continue;
                    }
                    let diagnostic = parsed
                        .diagnostics()
                        .iter()
                        .find(|d| d.code.as_deref() == Some("E_PARSE") && d.message == message)
                        .unwrap_or_else(|| {
                            panic!(
                                "missing required child error: {fragment}: {:?}",
                                parsed.diagnostics()
                            )
                        });
                    let start = source.find(highlight).expect("literal diagnostic slice");
                    let span = diagnostic.span.expect("owned diagnostic span");
                    assert_eq!(
                        (span.start as usize, span.end as usize),
                        (start, start + highlight.len())
                    );
                    let start = source.find(node).expect("literal recovered node");
                    assert!(
                        parsed.syntax_node().descendants().any(|n| n.kind() == kind
                            && u32::from(n.text_range().start()) as usize == start
                            && n.to_string() == node),
                        "retained complete syntax {node}"
                    );
                }
            }
        }
    }
    for source in [
        "fn run(value) {} struct Cell { value } enum Choice { One(value), Record { value } }",
        "trait Task { fn run(value); } impl Cell { fn run(value) {} }",
        "fn run() { let f = |value| value; value.; call(value = ); Unknown { value: }; }",
        "use module::value as kept; fn run(value: Array<i64>) {}",
    ] {
        assert!(
            parse_source(source).diagnostics().is_empty(),
            "unchanged optional/quiet recovery {source}"
        );
    }
}
