use crate::parse::parse_source;

#[test]
fn missing_required_declaration_parts_are_diagnosed_without_discarding_recovered_syntax() {
    for (fragment, message) in [
        ("const = 1;", "expected const name"),
        ("const Broken: i64 = ;", "expected const initializer"),
        (
            "state Broken: i64 = ;",
            "expected state initializer expression",
        ),
        ("fn Broken()", "expected function body"),
        ("fn Broken(: i64, value: i64) {}", "expected parameter name"),
        ("struct Broken { : i64, value: i64 }", "expected field name"),
        (
            "trait Broken { fn (self); fn read(self); }",
            "expected method name",
        ),
        (
            "impl Broken { fn (self) {} fn read(self) {} }",
            "expected method name",
        ),
        ("impl { fn ghost(self) {} }", "expected impl target"),
        (
            "fn Broken(value: i64 = ) {}",
            "expected parameter default expression",
        ),
        (
            "struct Broken { value: i64 = }",
            "expected field default expression",
        ),
        ("const Broken: = 1;", "expected type annotation"),
        ("fn Broken() -> {}", "expected return type"),
    ] {
        for newline in ["\n", "\r\n"] {
            let text = format!("/* 中😀 */ fn before() {{}}{newline}{fragment}");
            let parsed = parse_source(&text);
            assert_eq!(parsed.syntax_node().to_string(), text);
            let diagnostic = parsed
                .diagnostics()
                .iter()
                .find(|d| d.message == message && d.code.as_deref() == Some("E_PARSE"))
                .unwrap_or_else(|| {
                    panic!("missing {message}: {fragment}: {:?}", parsed.diagnostics())
                });
            let span = diagnostic.span.expect("owned source span");
            assert!(span.start as usize >= text.find(fragment).expect("fragment"));
            assert!(span.start < span.end && span.end as usize <= text.len());
            assert!(
                text.is_char_boundary(span.start as usize)
                    && text.is_char_boundary(span.end as usize)
            );
        }
    }
    for source in [
        "const LIMIT = 1; state value: i64 = 1; extern state live: i64;",
        "fn read(value: i64 = 1) { let row = Row { value }; row.value; row.0; read(value = 1); }",
        "struct Row { value: i64 } enum Choice { Pair(value: i64) }",
        "trait Read { fn read(self); } impl Row { fn read(self) {} }",
        "fn read(value: Array<i64>) { match value { Row { value } => 1 } }",
        "fn read() { let value:Map<String,i64>={\"key\":1}; }",
    ] {
        assert!(
            parse_source(source).diagnostics().is_empty(),
            "valid counterpart {source}"
        );
    }
}

#[test]
fn partial_expression_input_retains_the_accepted_quiet_recovery_policy() {
    for body in [
        "value.;",
        "call(value = );",
        "Unknown { value: };",
        "match value { Unknown { field: } => 1 }",
        "let value = ;",
    ] {
        for newline in ["\n", "\r\n"] {
            let text = format!("/* 中😀 */ fn before() {{}}{newline}fn partial() {{ {body} }}");
            let parsed = parse_source(&text);
            assert_eq!(parsed.syntax_node().to_string(), text);
            assert!(
                parsed.diagnostics().is_empty(),
                "quiet partial {body}: {:?}",
                parsed.diagnostics()
            );
        }
    }
    let parsed = parse_source("fn broken(");
    assert_eq!(parsed.diagnostics().len(), 1);
    assert_eq!(parsed.diagnostics()[0].message, "expected `)`");
    assert_eq!(parsed.diagnostics()[0].code.as_deref(), Some("E_PARSE"));
}

#[test]
fn recovered_header_names_do_not_borrow_later_identifiers() {
    use crate::ast::{
        AstNode, SyntaxConstItem, SyntaxEnumItem, SyntaxFunctionItem, SyntaxStateItem,
        SyntaxStructItem, SyntaxTraitItem, SyntaxUseItem,
    };
    for source in [
        "pub fn () -> Ghost",
        "pub fn = Ghost() {}",
        "pub struct = Ghost",
        "pub enum = Ghost",
        "pub trait = Ghost",
        "pub const : Ghost = 1;",
        "pub state :: Ghost: i64 = 1;",
        "pub extern state :: Ghost: i64;",
        "use module::value as = Ghost;",
    ] {
        let parsed = parse_source(source);
        assert_eq!(parsed.syntax_node().to_string(), source);
        let item = parsed.tree().items().next().expect("recovered declaration");
        let node = item.syntax().clone();
        let name = match node.kind() {
            crate::SyntaxKind::FunctionItem => SyntaxFunctionItem::cast(node)
                .expect("function")
                .name_token(),
            crate::SyntaxKind::StructItem => {
                SyntaxStructItem::cast(node).expect("struct").name_token()
            }
            crate::SyntaxKind::EnumItem => SyntaxEnumItem::cast(node).expect("enum").name_token(),
            crate::SyntaxKind::TraitItem => {
                SyntaxTraitItem::cast(node).expect("trait").name_token()
            }
            crate::SyntaxKind::ConstItem => {
                SyntaxConstItem::cast(node).expect("const").name_token()
            }
            crate::SyntaxKind::StateItem => {
                SyntaxStateItem::cast(node).expect("state").name_token()
            }
            crate::SyntaxKind::UseItem => SyntaxUseItem::cast(node).expect("use").alias_token(),
            kind => panic!("unexpected recovered item {kind:?}"),
        };
        assert!(name.is_none(), "borrowed header name in {source}: {name:?}");
    }
}

#[test]
fn named_function_without_body_retains_its_available_return_hint() {
    use crate::ast::{AstNode, SyntaxFunctionItem};
    for newline in ["\n", "\r\n"] {
        let source = format!("/* 中😀 */ fn before() {{}}{newline}pub fn Broken() -> Ghost");
        let parsed = parse_source(&source);
        assert_eq!(parsed.syntax_node().to_string(), source);
        let node = parsed
            .tree()
            .items()
            .nth(1)
            .expect("partial function item")
            .syntax()
            .clone();
        let function = SyntaxFunctionItem::cast(node).expect("function");
        assert_eq!(function.name_text().as_deref(), Some("Broken"));
        assert_eq!(
            function
                .return_type()
                .expect("available return hint")
                .syntax()
                .to_string(),
            "Ghost"
        );
        assert!(function.body().is_none());
        assert!(
            parsed
                .diagnostics()
                .iter()
                .any(|d| d.message == "expected function body")
        );
    }
}
