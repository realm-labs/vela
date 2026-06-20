use crate::ast::{AstNode, SyntaxSourceFile};
use crate::parse::parse_source;
use crate::{SyntaxKind, SyntaxTreeBuilder};

#[test]
fn ast_source_file_iterates_item_children() {
    let mut builder = SyntaxTreeBuilder::default();
    builder.start_node(SyntaxKind::SourceFile);
    builder.start_node(SyntaxKind::FunctionItem);
    builder.token(SyntaxKind::FnKw, "fn");
    builder.finish_node();
    builder.token(SyntaxKind::Whitespace, "\n");
    builder.start_node(SyntaxKind::StructItem);
    builder.token(SyntaxKind::StructKw, "struct");
    builder.finish_node();
    builder.finish_node();

    let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
    let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");

    assert_eq!(
        source
            .items()
            .map(|item| item.syntax().kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::FunctionItem, SyntaxKind::StructItem]
    );
}

#[test]
fn ast_items_expose_visibility_tokens() {
    let source = r#"pub use game::reward::grant;
pub const MAX: i64 = 10;
pub global state: ServerState;
#[event("tick")]
pub fn update() {}
pub struct Reward {}
pub enum Status {}
pub trait Award {}
pub impl Reward {}
fn private() {}
"#;
    let parse = parse_source(source);
    let tree = parse.tree();
    let items = tree.items().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(
        items
            .iter()
            .map(|item| item.is_public())
            .collect::<Vec<_>>(),
        vec![true, true, true, true, true, true, true, true, false]
    );
    assert_eq!(
        items[3].pub_token().expect("function pub token").text(),
        "pub"
    );
    assert!(tree.uses().next().expect("use item").is_public());
    assert!(tree.consts().next().expect("const item").is_public());
    assert!(tree.globals().next().expect("global item").is_public());
    assert_eq!(
        tree.functions()
            .next()
            .expect("function item")
            .pub_token()
            .expect("function pub")
            .kind(),
        SyntaxKind::PubKw
    );
    assert!(tree.structs().next().expect("struct item").is_public());
    assert!(tree.enums().next().expect("enum item").is_public());
    assert!(tree.traits().next().expect("trait item").is_public());
    assert!(tree.impls().next().expect("impl item").is_public());
}

#[test]
fn ast_items_expose_declaration_name_tokens() {
    let source = r#"use game::reward::grant as grant_reward;
const MAX: i64 = 10;
global state: ServerState;
fn update(ctx, amount: i64) {}
struct Reward {
    #[doc("amount")]
    amount: i64,
    item,
}
enum Status {
    Pending,
    Active(count: i64),
    Done { reward: String, xp: i64 },
}
trait Award {
    fn award(self, amount: i64);
}
impl Reward {
    fn grant(self) {}
}
"#;
    let parse = parse_source(source);
    let tree = parse.tree();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());

    let use_item = tree.uses().next().expect("use item");
    let use_path = use_item.path().expect("use path");
    assert_eq!(use_path.path_text().as_deref(), Some("game::reward::grant"));
    assert_eq!(
        use_path
            .path_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec!["game", "::", "reward", "::", "grant"]
    );
    assert_eq!(use_path.path_segments(), vec!["game", "reward", "grant"]);
    assert_eq!(
        use_path
            .path_separator_tokens()
            .iter()
            .map(|token| token.kind())
            .collect::<Vec<_>>(),
        vec![SyntaxKind::ColonColon, SyntaxKind::ColonColon]
    );
    assert_eq!(use_item.alias_text().as_deref(), Some("grant_reward"));
    assert_eq!(
        use_item.alias_token().expect("use alias").kind(),
        SyntaxKind::Ident
    );

    assert_eq!(
        tree.consts()
            .next()
            .expect("const item")
            .name_text()
            .as_deref(),
        Some("MAX")
    );
    assert_eq!(
        tree.globals()
            .next()
            .expect("global item")
            .name_text()
            .as_deref(),
        Some("state")
    );

    let function = tree.functions().next().expect("function item");
    assert_eq!(function.name_text().as_deref(), Some("update"));
    assert_eq!(
        function
            .param_list()
            .expect("function params")
            .params()
            .map(|param| param.name_text().expect("param name"))
            .collect::<Vec<_>>(),
        vec!["ctx", "amount"]
    );

    let struct_item = tree.structs().next().expect("struct item");
    assert_eq!(struct_item.name_text().as_deref(), Some("Reward"));
    let field_list = struct_item.field_list().expect("struct fields");
    assert_eq!(field_list.l_brace_token().expect("struct open").text(), "{");
    assert_eq!(
        field_list.r_brace_token().expect("struct close").text(),
        "}"
    );
    assert_eq!(
        field_list
            .separator_tokens()
            .iter()
            .map(|token| token.text())
            .collect::<Vec<_>>(),
        vec![",", ","]
    );
    let fields = field_list.fields().collect::<Vec<_>>();
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name_text().expect("field name"))
            .collect::<Vec<_>>(),
        vec!["amount", "item"]
    );
    assert_eq!(
        fields[0].name_token().expect("field token").text(),
        "amount"
    );

    let enum_item = tree.enums().next().expect("enum item");
    assert_eq!(enum_item.name_text().as_deref(), Some("Status"));
    let variant_list = enum_item.variant_list().expect("enum variants");
    assert_eq!(variant_list.l_brace_token().expect("enum open").text(), "{");
    assert_eq!(
        variant_list.r_brace_token().expect("enum close").text(),
        "}"
    );
    assert_eq!(
        variant_list
            .separator_tokens()
            .iter()
            .map(|token| token.text())
            .collect::<Vec<_>>(),
        vec![",", ",", ","]
    );
    let variants = variant_list.variants().collect::<Vec<_>>();
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.name_text().expect("variant name"))
            .collect::<Vec<_>>(),
        vec!["Pending", "Active", "Done"]
    );
    let tuple_field_list = variants[1].tuple_field_list().expect("tuple fields");
    assert_eq!(
        tuple_field_list.l_paren_token().expect("tuple open").text(),
        "("
    );
    assert_eq!(
        tuple_field_list
            .r_paren_token()
            .expect("tuple close")
            .text(),
        ")"
    );
    assert!(tuple_field_list.separator_tokens().is_empty());
    let tuple_param = tuple_field_list.params().next().expect("tuple field");
    assert_eq!(tuple_param.name_text().as_deref(), Some("count"));
    let record_field_list = variants[2].record_field_list().expect("record fields");
    assert_eq!(
        record_field_list
            .l_brace_token()
            .expect("record open")
            .text(),
        "{"
    );
    assert_eq!(
        record_field_list
            .r_brace_token()
            .expect("record close")
            .text(),
        "}"
    );
    assert_eq!(
        record_field_list
            .separator_tokens()
            .iter()
            .map(|token| token.text())
            .collect::<Vec<_>>(),
        vec![","]
    );

    let trait_item = tree.traits().next().expect("trait item");
    assert_eq!(trait_item.name_text().as_deref(), Some("Award"));
    let trait_method = trait_item.methods().next().expect("trait method");
    assert_eq!(trait_method.name_text().as_deref(), Some("award"));
    assert_eq!(
        trait_method
            .param_list()
            .expect("trait method params")
            .params()
            .map(|param| param.name_text().expect("trait param name"))
            .collect::<Vec<_>>(),
        vec!["self", "amount"]
    );

    let impl_method = tree
        .impls()
        .next()
        .expect("impl item")
        .methods()
        .next()
        .expect("impl method");
    assert_eq!(impl_method.name_text().as_deref(), Some("grant"));
}

#[test]
fn ast_function_item_exposes_signature_and_body_children() {
    let mut builder = SyntaxTreeBuilder::default();
    builder.start_node(SyntaxKind::SourceFile);
    builder.start_node(SyntaxKind::FunctionItem);
    builder.token(SyntaxKind::FnKw, "fn");
    builder.start_node(SyntaxKind::ParamList);
    builder.token(SyntaxKind::LParen, "(");
    builder.start_node(SyntaxKind::Param);
    builder.token(SyntaxKind::Ident, "ctx");
    builder.finish_node();
    builder.token(SyntaxKind::Comma, ",");
    builder.start_node(SyntaxKind::Param);
    builder.token(SyntaxKind::Ident, "event");
    builder.finish_node();
    builder.token(SyntaxKind::RParen, ")");
    builder.finish_node();
    builder.start_node(SyntaxKind::Block);
    builder.token(SyntaxKind::LBrace, "{");
    builder.token(SyntaxKind::RBrace, "}");
    builder.finish_node();
    builder.finish_node();
    builder.finish_node();

    let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
    let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");
    let function = source.functions().next().expect("function item");
    let param_list = function.param_list().expect("param list");

    assert_eq!(
        param_list
            .params()
            .map(|param| param.syntax().text().to_string())
            .collect::<Vec<_>>(),
        vec!["ctx", "event"]
    );
    assert_eq!(param_list.l_paren_token().expect("open paren").text(), "(");
    assert_eq!(param_list.r_paren_token().expect("close paren").text(), ")");
    assert_eq!(
        param_list
            .separator_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec![","]
    );
    assert!(param_list.pipe_tokens().is_empty());
    assert_eq!(
        function.body().expect("body").syntax().text().to_string(),
        "{}"
    );
}

#[test]
fn ast_function_signature_exposes_type_hint_children() {
    let mut builder = SyntaxTreeBuilder::default();
    builder.start_node(SyntaxKind::SourceFile);
    builder.start_node(SyntaxKind::FunctionItem);
    builder.start_node(SyntaxKind::ParamList);
    builder.token(SyntaxKind::LParen, "(");
    builder.start_node(SyntaxKind::Param);
    builder.token(SyntaxKind::Ident, "items");
    builder.token(SyntaxKind::Colon, ":");
    builder.start_node(SyntaxKind::TypeHint);
    builder.token(SyntaxKind::Ident, "Array");
    builder.start_node(SyntaxKind::TypeArgList);
    builder.token(SyntaxKind::Less, "<");
    builder.token(SyntaxKind::Ident, "String");
    builder.token(SyntaxKind::Greater, ">");
    builder.finish_node();
    builder.finish_node();
    builder.finish_node();
    builder.token(SyntaxKind::RParen, ")");
    builder.finish_node();
    builder.token(SyntaxKind::Arrow, "->");
    builder.start_node(SyntaxKind::TypeHint);
    builder.token(SyntaxKind::Ident, "Result");
    builder.finish_node();
    builder.finish_node();
    builder.finish_node();

    let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
    let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");
    let function = source.functions().next().expect("function item");
    let param = function
        .param_list()
        .expect("param list")
        .params()
        .next()
        .expect("param");

    let hint = param.type_hint().expect("param type hint");
    assert_eq!(hint.syntax().text().to_string(), "Array<String>");
    assert_eq!(
        hint.type_arg_list()
            .expect("type arg list")
            .syntax()
            .text()
            .to_string(),
        "<String>"
    );
    assert_eq!(
        function
            .return_type()
            .expect("return type")
            .syntax()
            .text()
            .to_string(),
        "Result"
    );
}

#[test]
fn ast_type_hints_expose_path_and_argument_delimiters() {
    let parse = parse_source(
        r#"fn typed(items: Map<String, Result<i64, String>>) -> game::Reward {
    return null;
}

struct Bag {
    entries: Map<String, i64>,
}
"#,
    );
    let tree = parse.tree();
    let function = tree.functions().next().expect("function item");

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());

    let param = function
        .param_list()
        .expect("function params")
        .params()
        .next()
        .expect("param");
    let hint = param.type_hint().expect("param type hint");
    let args = hint.type_arg_list().expect("type args");
    let arg_hints = args.type_hints().collect::<Vec<_>>();

    assert_eq!(hint.path_text().as_deref(), Some("Map"));
    assert_eq!(hint.path_segments(), vec!["Map"]);
    assert!(hint.path_separator_tokens().is_empty());
    assert_eq!(
        hint.path_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec!["Map"]
    );
    assert_eq!(args.less_token().expect("less token").text(), "<");
    assert_eq!(args.greater_token().expect("greater token").text(), ">");
    assert_eq!(
        args.separator_tokens()
            .iter()
            .map(|token| token.text())
            .collect::<Vec<_>>(),
        vec![","]
    );
    assert_eq!(arg_hints[0].path_text().as_deref(), Some("String"));
    assert_eq!(arg_hints[0].path_segments(), vec!["String"]);
    assert!(arg_hints[0].path_separator_tokens().is_empty());
    assert_eq!(arg_hints[1].path_text().as_deref(), Some("Result"));
    assert_eq!(arg_hints[1].path_segments(), vec!["Result"]);
    assert!(arg_hints[1].path_separator_tokens().is_empty());

    let result_arg_list = arg_hints[1].type_arg_list().expect("result type args");
    assert_eq!(
        result_arg_list
            .separator_tokens()
            .iter()
            .map(|token| token.text())
            .collect::<Vec<_>>(),
        vec![","]
    );
    let result_args = result_arg_list.type_hints().collect::<Vec<_>>();
    assert_eq!(result_args[0].path_text().as_deref(), Some("i64"));
    assert_eq!(result_args[0].path_segments(), vec!["i64"]);
    assert_eq!(result_args[1].path_text().as_deref(), Some("String"));
    assert_eq!(result_args[1].path_segments(), vec!["String"]);

    let return_type = function.return_type().expect("return type");
    assert_eq!(return_type.path_text().as_deref(), Some("game::Reward"));
    assert_eq!(return_type.path_segments(), vec!["game", "Reward"]);
    assert_eq!(
        return_type
            .path_separator_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec!["::"]
    );
    assert_eq!(
        return_type
            .path_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec!["game", "::", "Reward"]
    );
    assert!(return_type.type_arg_list().is_none());

    let field = tree
        .structs()
        .next()
        .expect("struct item")
        .field_list()
        .expect("field list")
        .fields()
        .next()
        .expect("field");
    let field_hint = field.type_hint().expect("field type hint");
    assert_eq!(field.name_text().as_deref(), Some("entries"));
    assert_eq!(field_hint.path_text().as_deref(), Some("Map"));
    assert_eq!(
        field_hint
            .type_arg_list()
            .expect("field type args")
            .type_hints()
            .map(|hint| hint.path_text().expect("type arg path"))
            .collect::<Vec<_>>(),
        vec!["String", "i64"]
    );
}

#[test]
fn ast_items_expose_default_value_expressions() {
    let source = r#"
fn defaults(amount: i64 = bonus(1), label = "gold") -> i64 {
    return amount
}

struct Reward {
    amount: i64 = 10 + 5,
    label = "gold",
}

enum Status {
    Active(count: i64 = 1),
    Finished { reward: String = "gold" },
}
"#;
    let parse = parse_source(source);
    let tree = parse.tree();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());

    let function = tree.functions().next().expect("function item");
    let params = function
        .param_list()
        .expect("function params")
        .params()
        .collect::<Vec<_>>();
    assert_eq!(
        params[0].default_equal_token().map(|token| token.kind()),
        Some(SyntaxKind::Equal)
    );
    assert_eq!(
        params[0]
            .default_value()
            .expect("typed default")
            .syntax()
            .kind(),
        SyntaxKind::CallExpr
    );
    assert_eq!(
        params[0]
            .default_value()
            .expect("typed default")
            .syntax()
            .text()
            .to_string(),
        "bonus(1)"
    );
    assert_eq!(
        params[1]
            .default_value()
            .expect("untyped default")
            .syntax()
            .kind(),
        SyntaxKind::Literal
    );

    let struct_fields = tree
        .structs()
        .next()
        .expect("struct item")
        .field_list()
        .expect("struct fields")
        .fields()
        .collect::<Vec<_>>();
    assert_eq!(
        struct_fields[0]
            .default_value()
            .expect("field default")
            .syntax()
            .kind(),
        SyntaxKind::BinaryExpr
    );
    assert_eq!(
        struct_fields[1]
            .default_equal_token()
            .map(|token| token.kind()),
        Some(SyntaxKind::Equal)
    );

    let variants = tree
        .enums()
        .next()
        .expect("enum item")
        .variant_list()
        .expect("variant list")
        .variants()
        .collect::<Vec<_>>();
    let tuple_param = variants[0]
        .tuple_field_list()
        .expect("tuple fields")
        .params()
        .next()
        .expect("tuple param");
    assert_eq!(
        tuple_param
            .default_value()
            .expect("tuple default")
            .syntax()
            .text()
            .to_string(),
        "1"
    );

    let record_field = variants[1]
        .record_field_list()
        .expect("record fields")
        .fields()
        .next()
        .expect("record field");
    assert_eq!(
        record_field
            .default_value()
            .expect("record default")
            .syntax()
            .text()
            .to_string(),
        "\"gold\""
    );
}

#[test]
fn ast_struct_item_exposes_field_children() {
    let mut builder = SyntaxTreeBuilder::default();
    builder.start_node(SyntaxKind::SourceFile);
    builder.start_node(SyntaxKind::StructItem);
    builder.token(SyntaxKind::StructKw, "struct");
    builder.start_node(SyntaxKind::StructFieldList);
    builder.token(SyntaxKind::LBrace, "{");
    builder.start_node(SyntaxKind::StructField);
    builder.token(SyntaxKind::Ident, "items");
    builder.token(SyntaxKind::Colon, ":");
    builder.start_node(SyntaxKind::TypeHint);
    builder.token(SyntaxKind::Ident, "Array");
    builder.start_node(SyntaxKind::TypeArgList);
    builder.token(SyntaxKind::Less, "<");
    builder.token(SyntaxKind::Ident, "String");
    builder.token(SyntaxKind::Greater, ">");
    builder.finish_node();
    builder.finish_node();
    builder.finish_node();
    builder.token(SyntaxKind::Comma, ",");
    builder.start_node(SyntaxKind::StructField);
    builder.token(SyntaxKind::Ident, "count");
    builder.finish_node();
    builder.token(SyntaxKind::RBrace, "}");
    builder.finish_node();
    builder.finish_node();
    builder.finish_node();

    let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
    let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");
    let record = source.structs().next().expect("struct item");
    let fields = record
        .field_list()
        .expect("field list")
        .fields()
        .collect::<Vec<_>>();

    assert_eq!(
        fields
            .iter()
            .map(|field| field.syntax().text().to_string())
            .collect::<Vec<_>>(),
        vec!["items:Array<String>", "count"]
    );
    let hint = fields[0].type_hint().expect("field type hint");
    assert_eq!(hint.syntax().text().to_string(), "Array<String>");
    assert_eq!(
        hint.type_arg_list()
            .expect("field type args")
            .syntax()
            .text()
            .to_string(),
        "<String>"
    );
    assert!(fields[1].type_hint().is_none());
}

#[test]
fn ast_enum_item_exposes_variant_children() {
    let mut builder = SyntaxTreeBuilder::default();
    builder.start_node(SyntaxKind::SourceFile);
    builder.start_node(SyntaxKind::EnumItem);
    builder.token(SyntaxKind::EnumKw, "enum");
    builder.start_node(SyntaxKind::EnumVariantList);
    builder.token(SyntaxKind::LBrace, "{");
    builder.start_node(SyntaxKind::EnumVariant);
    builder.token(SyntaxKind::Ident, "Finished");
    builder.start_node(SyntaxKind::TupleFieldList);
    builder.token(SyntaxKind::LParen, "(");
    builder.start_node(SyntaxKind::Param);
    builder.token(SyntaxKind::Ident, "reward");
    builder.token(SyntaxKind::Colon, ":");
    builder.start_node(SyntaxKind::TypeHint);
    builder.token(SyntaxKind::Ident, "Option");
    builder.start_node(SyntaxKind::TypeArgList);
    builder.token(SyntaxKind::Less, "<");
    builder.token(SyntaxKind::Ident, "String");
    builder.token(SyntaxKind::Greater, ">");
    builder.finish_node();
    builder.finish_node();
    builder.finish_node();
    builder.token(SyntaxKind::RParen, ")");
    builder.finish_node();
    builder.finish_node();
    builder.token(SyntaxKind::Comma, ",");
    builder.start_node(SyntaxKind::EnumVariant);
    builder.token(SyntaxKind::Ident, "Active");
    builder.start_node(SyntaxKind::RecordFieldList);
    builder.token(SyntaxKind::LBrace, "{");
    builder.start_node(SyntaxKind::StructField);
    builder.token(SyntaxKind::Ident, "count");
    builder.token(SyntaxKind::Colon, ":");
    builder.start_node(SyntaxKind::TypeHint);
    builder.token(SyntaxKind::Ident, "i64");
    builder.finish_node();
    builder.finish_node();
    builder.token(SyntaxKind::RBrace, "}");
    builder.finish_node();
    builder.finish_node();
    builder.token(SyntaxKind::RBrace, "}");
    builder.finish_node();
    builder.finish_node();
    builder.finish_node();

    let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
    let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");
    let enumeration = source.enums().next().expect("enum item");
    let variants = enumeration
        .variant_list()
        .expect("variant list")
        .variants()
        .collect::<Vec<_>>();

    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.syntax().text().to_string())
            .collect::<Vec<_>>(),
        vec!["Finished(reward:Option<String>)", "Active{count:i64}"]
    );
    let tuple_param = variants[0]
        .tuple_field_list()
        .expect("tuple fields")
        .params()
        .next()
        .expect("tuple param");
    let tuple_hint = tuple_param.type_hint().expect("tuple param type");
    assert_eq!(tuple_hint.syntax().text().to_string(), "Option<String>");
    assert_eq!(
        tuple_hint
            .type_arg_list()
            .expect("tuple type args")
            .syntax()
            .text()
            .to_string(),
        "<String>"
    );
    let record_field = variants[1]
        .record_field_list()
        .expect("record fields")
        .fields()
        .next()
        .expect("record field");
    assert_eq!(
        record_field
            .type_hint()
            .expect("record field type")
            .syntax()
            .text()
            .to_string(),
        "i64"
    );
}

#[test]
fn ast_trait_and_impl_items_expose_method_children() {
    let mut builder = SyntaxTreeBuilder::default();
    builder.start_node(SyntaxKind::SourceFile);
    builder.start_node(SyntaxKind::TraitItem);
    builder.token(SyntaxKind::TraitKw, "trait");
    builder.token(SyntaxKind::LBrace, "{");
    builder.start_node(SyntaxKind::TraitMethod);
    builder.token(SyntaxKind::FnKw, "fn");
    builder.token(SyntaxKind::Ident, "reward");
    builder.start_node(SyntaxKind::ParamList);
    builder.token(SyntaxKind::LParen, "(");
    builder.start_node(SyntaxKind::Param);
    builder.token(SyntaxKind::Ident, "amount");
    builder.token(SyntaxKind::Colon, ":");
    builder.start_node(SyntaxKind::TypeHint);
    builder.token(SyntaxKind::Ident, "i64");
    builder.finish_node();
    builder.finish_node();
    builder.token(SyntaxKind::RParen, ")");
    builder.finish_node();
    builder.token(SyntaxKind::Arrow, "->");
    builder.start_node(SyntaxKind::TypeHint);
    builder.token(SyntaxKind::Ident, "String");
    builder.finish_node();
    builder.token(SyntaxKind::Semicolon, ";");
    builder.finish_node();
    builder.token(SyntaxKind::RBrace, "}");
    builder.finish_node();
    builder.start_node(SyntaxKind::ImplItem);
    builder.token(SyntaxKind::ImplKw, "impl");
    builder.token(SyntaxKind::LBrace, "{");
    builder.start_node(SyntaxKind::ImplMethod);
    builder.token(SyntaxKind::FnKw, "fn");
    builder.token(SyntaxKind::Ident, "reward");
    builder.start_node(SyntaxKind::ParamList);
    builder.token(SyntaxKind::LParen, "(");
    builder.token(SyntaxKind::RParen, ")");
    builder.finish_node();
    builder.start_node(SyntaxKind::Block);
    builder.token(SyntaxKind::LBrace, "{");
    builder.token(SyntaxKind::ReturnKw, "return");
    builder.token(SyntaxKind::String, "\"gold\"");
    builder.token(SyntaxKind::Semicolon, ";");
    builder.token(SyntaxKind::RBrace, "}");
    builder.finish_node();
    builder.finish_node();
    builder.token(SyntaxKind::RBrace, "}");
    builder.finish_node();
    builder.finish_node();

    let parse: crate::Parse<SyntaxSourceFile> = builder.finish();
    let source = SyntaxSourceFile::cast(parse.syntax_node()).expect("source file root");
    let trait_item = source.traits().next().expect("trait item");
    let trait_method = trait_item.methods().next().expect("trait method");
    let impl_item = source.impls().next().expect("impl item");
    let impl_method = impl_item.methods().next().expect("impl method");

    assert_eq!(
        trait_method.syntax().text().to_string(),
        "fnreward(amount:i64)->String;"
    );
    assert_eq!(
        trait_method
            .param_list()
            .expect("trait params")
            .params()
            .next()
            .expect("trait param")
            .type_hint()
            .expect("param type")
            .syntax()
            .text()
            .to_string(),
        "i64"
    );
    assert_eq!(
        trait_method
            .return_type()
            .expect("trait return type")
            .syntax()
            .text()
            .to_string(),
        "String"
    );
    assert!(trait_method.body().is_none());
    assert_eq!(
        impl_method
            .body()
            .expect("impl body")
            .syntax()
            .text()
            .to_string(),
        "{return\"gold\";}"
    );
}

#[test]
fn ast_impl_item_exposes_header_paths() {
    let source = r#"trait Rewardable {}

impl Reward {
    fn grant(self) {}
}

impl game::reward::Rewardable for game::player::Player {
    fn reward(self) {}
}
"#;
    let parse = parse_source(source);
    let tree = parse.tree();
    let impls = tree.impls().collect::<Vec<_>>();

    assert!(parse.diagnostics().is_empty(), "{:?}", parse.diagnostics());
    assert_eq!(impls.len(), 2);

    let inherent = &impls[0];
    assert_eq!(inherent.impl_token().expect("impl token").text(), "impl");
    assert!(inherent.for_token().is_none());
    assert!(inherent.trait_path_text().is_none());
    assert!(inherent.trait_path_tokens().is_empty());
    assert!(inherent.trait_path_segments().is_empty());
    assert_eq!(inherent.target_path_text().as_deref(), Some("Reward"));
    assert_eq!(inherent.target_path_segments(), vec!["Reward"]);
    assert_eq!(
        inherent
            .target_path_tokens()
            .iter()
            .map(|token| (token.kind(), token.text().to_owned()))
            .collect::<Vec<_>>(),
        vec![(SyntaxKind::Ident, "Reward".to_owned())]
    );

    let trait_impl = &impls[1];
    assert_eq!(trait_impl.impl_token().expect("impl token").text(), "impl");
    assert_eq!(trait_impl.for_token().expect("for token").text(), "for");
    assert_eq!(
        trait_impl.trait_path_text().as_deref(),
        Some("game::reward::Rewardable")
    );
    assert_eq!(
        trait_impl.target_path_text().as_deref(),
        Some("game::player::Player")
    );
    assert_eq!(
        trait_impl.trait_path_segments(),
        vec!["game", "reward", "Rewardable"]
    );
    assert_eq!(
        trait_impl.target_path_segments(),
        vec!["game", "player", "Player"]
    );
    assert_eq!(
        trait_impl
            .trait_path_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec!["game", "::", "reward", "::", "Rewardable"]
    );
    assert_eq!(
        trait_impl
            .target_path_tokens()
            .iter()
            .map(|token| token.text().to_owned())
            .collect::<Vec<_>>(),
        vec!["game", "::", "player", "::", "Player"]
    );
    assert_eq!(trait_impl.methods().count(), 1);
}
