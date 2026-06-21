use super::*;
use crate::{
    SourceFileSnapshot, Workspace, WorkspaceConfig, WorkspaceRoot, assemble_project_sources,
};

fn project_databases(source: &str) -> (LanguageServiceDatabases, DocumentId) {
    let document_id = DocumentId::from("file:///workspace/scripts/main.vela");
    let config = WorkspaceConfig::workspace([WorkspaceRoot::new("/workspace/scripts")]);
    let files = vec![SourceFileSnapshot::new(document_id.clone(), source)];
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);

    (databases, document_id)
}

fn format_source(source: &str) -> Vec<TextEdit> {
    let (databases, document_id) = project_databases(source);
    databases.document_formatting(&document_id)
}

fn range_format_source(source: &str, range: DiagnosticRange) -> Vec<TextEdit> {
    let (databases, document_id) = project_databases(source);
    databases.range_formatting(&document_id, range)
}

fn on_type_format_source(source: &str, position: Position, trigger: &str) -> Vec<TextEdit> {
    let (databases, document_id) = project_databases(source);
    databases.on_type_formatting(&document_id, position, trigger)
}

fn formatting_ir(source: &str) -> FormattingIr {
    let (databases, document_id) = project_databases(source);
    databases
        .formatting_ir(&document_id)
        .expect("formatting IR should be available")
}

fn apply_edits(source: &str, edits: &[TextEdit]) -> String {
    if edits.is_empty() {
        return source.to_owned();
    }
    assert_eq!(edits.len(), 1);
    edits[0].new_text().to_owned()
}

fn apply_range_edits(source: &str, edits: &[TextEdit]) -> String {
    let line_index = LineIndex::new(source);
    let mut formatted = source.to_owned();
    let mut offset_edits = edits
        .iter()
        .map(|edit| {
            (
                line_index.offset(edit.range().start()),
                line_index.offset(edit.range().end()),
                edit.new_text().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    offset_edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    for (start, end, replacement) in offset_edits {
        formatted.replace_range(start..end, &replacement);
    }
    formatted
}

#[test]
fn formatting_preserves_comments() {
    let source = "// keep this comment   \npub fn main() { // inline\t\n    return 1   \n}";
    let edits = format_source(source);
    let formatted = apply_edits(source, &edits);

    assert_eq!(
        formatted,
        "// keep this comment\npub fn main() {\n    // inline\n    return 1\n}\n"
    );
}

#[test]
fn formatting_is_idempotent() {
    let source = "pub fn main() {\n    return 1\n}\n";
    let edits = format_source(source);

    assert!(edits.is_empty());
}

#[test]
fn formatting_handles_malformed_source_without_panic() {
    let source = "pub fn main( {   ";
    let edits = format_source(source);
    let formatted = apply_edits(source, &edits);

    assert_eq!(formatted, "pub fn main( {\n");
}

#[test]
fn formatting_does_not_depend_on_successful_hir_analysis() {
    let document_id = DocumentId::from("/workspace/scripts/game/main.vela");
    let source = "use game::reward::grant_bonus\npub fn main(){return 1}";
    let config = WorkspaceConfig::workspace([WorkspaceRoot::new("/workspace/scripts")]);
    let files = vec![
        SourceFileSnapshot::new(document_id.clone(), source),
        SourceFileSnapshot::new(
            "/workspace/scripts/game/reward.vela",
            "pub fn grant() { return 1 }",
        ),
    ];
    let project = assemble_project_sources(&config, &files, &Workspace::new().snapshot());
    let mut databases = LanguageServiceDatabases::new();
    databases.update(&project);

    let diagnostics = databases.diagnostics_for_document(&document_id);
    assert!(
        diagnostics
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == Some("hir::unresolved_import")),
        "fixture should keep a semantic analysis error"
    );

    let edits = databases.document_formatting(&document_id);
    let formatted = apply_edits(source, &edits);

    assert_eq!(
        formatted,
        "\
use game::reward::grant_bonus
pub fn main() {
    return 1
}
"
    );
}

#[test]
fn formatting_formats_item_declarations() {
    let source = "pub struct Player{level:i64 name:String}impl Player{fn heal(amount:i64)->i64{return amount}}";
    let edits = format_source(source);
    let formatted = apply_edits(source, &edits);

    assert_eq!(
        formatted,
        "\
pub struct Player {
    level: i64
    name: String
}
impl Player {
    fn heal(amount: i64) -> i64 {
        return amount
    }
}
"
    );
}

#[test]
fn range_formatting_limits_edits_to_range() {
    let source = "pub fn main() {   \n    return 1   \n}\n";
    let edits = range_format_source(
        source,
        DiagnosticRange::new(Position::new(1, 0), Position::new(2, 0)),
    );
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(1, 12));
    assert_eq!(edits[0].range().end(), Position::new(1, 15));
    assert_eq!(formatted, "pub fn main() {   \n    return 1\n}\n");
}

#[test]
fn range_formatting_formats_selected_item() {
    let source = "pub fn main(){return 1}\n\npub fn other(){return 2}\n";
    let edits = range_format_source(
        source,
        DiagnosticRange::new(Position::new(0, 0), Position::new(1, 0)),
    );
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(0, 0));
    assert_eq!(edits[0].range().end(), Position::new(1, 0));
    assert_eq!(
        formatted,
        "\
pub fn main() {
    return 1
}

pub fn other(){return 2}
"
    );
}

#[test]
fn range_formatting_formats_item_with_leading_blank_selection() {
    let source = "\n\npub fn main(){return 1}\n\npub fn other(){return 2}\n";
    let edits = range_format_source(
        source,
        DiagnosticRange::new(Position::new(0, 0), Position::new(3, 0)),
    );
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(2, 0));
    assert_eq!(edits[0].range().end(), Position::new(3, 0));
    assert_eq!(
        formatted,
        "\n\npub fn main() {\n    return 1\n}\n\npub fn other(){return 2}\n"
    );
}

#[test]
fn range_formatting_formats_selected_impl_method() {
    let source = "impl Player{fn heal(amount:i64)->i64{return amount}fn hurt(amount:i64)->i64{return amount}}\n";
    let edits = range_format_source(
        source,
        DiagnosticRange::new(Position::new(0, 12), Position::new(0, 51)),
    );
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(0, 12));
    assert_eq!(edits[0].range().end(), Position::new(0, 51));
    assert_eq!(
        formatted,
        "impl Player{fn heal(amount: i64) -> i64 {\n    return amount\n}\nfn hurt(amount:i64)->i64{return amount}}\n"
    );
}

#[test]
fn range_formatting_formats_selected_trait_method() {
    let source = "pub trait Rewardable{fn preview(amount:i64)->i64 fn other(amount:i64)->i64}\n";
    let start = source.find("fn preview").expect("selected method");
    let end = start + "fn preview(amount:i64)->i64".len();
    let edits = range_format_source(
        source,
        DiagnosticRange::new(Position::new(0, start), Position::new(0, end)),
    );
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(0, start));
    assert_eq!(edits[0].range().end(), Position::new(0, end));
    assert_eq!(
        formatted,
        "pub trait Rewardable{fn preview(amount: i64) -> i64 fn other(amount:i64)->i64}\n"
    );
}

#[test]
fn range_formatting_preserves_nested_method_indent() {
    let source = "\
impl Player {
    fn heal(amount:i64)->i64{return amount}
    fn hurt(amount:i64)->i64{return amount}
}
";
    let edits = range_format_source(
        source,
        DiagnosticRange::new(Position::new(1, 4), Position::new(1, 43)),
    );
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(1, 4));
    assert_eq!(edits[0].range().end(), Position::new(1, 43));
    assert_eq!(
        formatted,
        "impl Player {\n    fn heal(amount: i64) -> i64 {\n        return amount\n    }\n    fn hurt(amount:i64)->i64{return amount}\n}\n"
    );
}

#[test]
fn range_formatting_preserves_struct_field_indent() {
    let source = "\
pub struct Player {
    level:i64
    name:String
}
";
    let edits = range_format_source(
        source,
        DiagnosticRange::new(Position::new(1, 4), Position::new(1, 13)),
    );
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(1, 4));
    assert_eq!(edits[0].range().end(), Position::new(1, 13));
    assert_eq!(
        formatted,
        "\
pub struct Player {
    level: i64
    name:String
}
"
    );
}

#[test]
fn range_formatting_formats_selected_struct_field_group() {
    let source = "\
pub struct Player {
    level:i64
    name:String
    xp:i64
}
";
    let edits = range_format_source(
        source,
        DiagnosticRange::new(Position::new(1, 4), Position::new(2, 15)),
    );
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(1, 4));
    assert_eq!(edits[0].range().end(), Position::new(2, 15));
    assert_eq!(
        formatted,
        "\
pub struct Player {
    level: i64
    name: String
    xp:i64
}
"
    );
}

#[test]
fn range_formatting_formats_selected_enum_record_field_group() {
    let source = "\
pub enum Reward {
    Coins {
        amount:i64
        label:String
        rare:bool
    }
    None
}
";
    let edits = range_format_source(
        source,
        DiagnosticRange::new(Position::new(2, 8), Position::new(3, 20)),
    );
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(2, 8));
    assert_eq!(edits[0].range().end(), Position::new(3, 20));
    assert_eq!(
        formatted,
        "\
pub enum Reward {
    Coins {
        amount: i64
        label: String
        rare:bool
    }
    None
}
"
    );
}

#[test]
fn on_type_formatting_only_edits_current_construct() {
    let source = concat!(
        "pub fn main() {   \n",
        "    return 1   \n",
        "}\n",
        "\n",
        "pub fn other() {   \n",
        "    return 2   \n",
        "}\n",
    );
    let edits = on_type_format_source(source, Position::new(2, 1), "}");
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(0, 0));
    assert_eq!(edits[0].range().end(), Position::new(3, 0));
    assert_eq!(
        formatted,
        concat!(
            "pub fn main() {\n",
            "    return 1\n",
            "}\n",
            "\n",
            "pub fn other() {   \n",
            "    return 2   \n",
            "}\n",
        )
    );
}

#[test]
fn on_type_formatting_reflows_completed_item() {
    let source = "pub fn main(){return 1}\n\npub fn other(){return 2}\n";
    let edits = on_type_format_source(source, Position::new(0, 23), "}");
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(0, 0));
    assert_eq!(edits[0].range().end(), Position::new(1, 0));
    assert_eq!(
        formatted,
        "\
pub fn main() {
    return 1
}

pub fn other(){return 2}
"
    );
}

#[test]
fn on_type_formatting_reflows_completed_multiline_item() {
    let source = "\
pub fn main(){
    return 1+2
}

pub fn other(){return 2}
";
    let edits = on_type_format_source(source, Position::new(2, 1), "}");
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(0, 0));
    assert_eq!(edits[0].range().end(), Position::new(3, 0));
    assert_eq!(
        formatted,
        "\
pub fn main() {
    return 1 + 2
}

pub fn other(){return 2}
"
    );
}

#[test]
fn on_type_formatting_reflows_completed_nested_method() {
    let source = "\
impl Player {
    fn heal(amount:i64)->i64{return amount}
    fn hurt(amount:i64)->i64{return amount}
}
";
    let edits = on_type_format_source(source, Position::new(1, 43), "}");
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(1, 4));
    assert_eq!(edits[0].range().end(), Position::new(2, 0));
    assert_eq!(
        formatted,
        "\
impl Player {
    fn heal(amount: i64) -> i64 {
        return amount
    }
    fn hurt(amount:i64)->i64{return amount}
}
"
    );
}

#[test]
fn on_type_formatting_reflows_completed_enum_record_variant() {
    let source = "\
pub enum Reward {
    Coins {
        amount:i64
        label:String
    }
    None
}
";
    let edits = on_type_format_source(source, Position::new(4, 5), "}");
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range().start(), Position::new(1, 4));
    assert_eq!(edits[0].range().end(), Position::new(5, 0));
    assert_eq!(
        formatted,
        "\
pub enum Reward {
    Coins {
        amount: i64
        label: String
    }
    None
}
"
    );
}

#[test]
fn on_type_formatting_ignores_unsupported_trigger() {
    let source = "pub fn main() {   \n    return 1   \n}\n";
    let edits = on_type_format_source(source, Position::new(0, 16), "(");

    assert!(edits.is_empty());
}

#[test]
fn formatting_ir_preserves_comments_and_blank_line_groups() {
    let source = "#!/usr/bin/env vela\n\npub fn main() {\n    /* keep\n\n       grouped */\n    // tail\n    return 1\n}\n";
    let ir = formatting_ir(source);
    let comment_texts = ir
        .segments()
        .iter()
        .filter(|segment| {
            matches!(
                segment.kind(),
                FormattingSegmentKind::LineComment | FormattingSegmentKind::BlockComment
            )
        })
        .map(FormattingSegment::text)
        .collect::<Vec<_>>();
    let preserves_blank_line_group = ir.segments().windows(2).any(|segments| {
        segments[0].kind() == FormattingSegmentKind::Shebang
            && segments[1].kind() == FormattingSegmentKind::Whitespace
            && format!("{}{}", segments[0].text(), segments[1].text())
                .matches('\n')
                .count()
                >= 2
    });

    assert_eq!(
        ir.document_id().as_str(),
        "file:///workspace/scripts/main.vela"
    );
    assert_eq!(ir.reconstruct_source(), source);
    assert_eq!(
        comment_texts,
        vec!["/* keep\n\n       grouped */", "// tail"]
    );
    assert!(preserves_blank_line_group);
    assert_eq!(ir.segments()[0].kind(), FormattingSegmentKind::Shebang);
    assert_eq!(ir.segments()[0].range().start(), Position::new(0, 0));
}

#[test]
fn formatting_formats_container_type_hint_example() {
    let source = "\
fn load_rewards(rewards:Map < String,i64 >)->Result < Map<String , i64>,String >{return result::ok(rewards)}

fn main(){let scores:Array < i64 > = [1,2,3];let rewards:Map < String,i64 >={\"xp\":5};let tags:Set < String > = set::from_array([\"daily\",\"vip\"]);return score(scores,rewards,tags).unwrap_or(0)}
";
    let edits = format_source(source);
    let formatted = apply_edits(source, &edits);

    assert_eq!(
        formatted,
        "\
fn load_rewards(rewards: Map<String, i64>) -> Result<Map<String, i64>, String> {
    return result::ok(rewards)
}

fn main() {
    let scores: Array<i64> = [1, 2, 3];
    let rewards: Map<String, i64> = {
        \"xp\": 5
    };
    let tags: Set<String> = set::from_array([\"daily\", \"vip\"]);
    return score(scores, rewards, tags).unwrap_or(0)
}
"
    );
}

#[test]
fn formatting_handles_incomplete_container_type_arguments() {
    let source = "fn load_rewards(rewards:Map < String, ){return rewards}";
    let edits = format_source(source);
    let formatted = apply_edits(source, &edits);

    assert_eq!(
        formatted,
        "\
fn load_rewards(rewards: Map<String,) {
    return rewards
}
"
    );
}

#[test]
fn range_formatting_compacts_builtin_container_type_arguments() {
    let source = "\
fn load_rewards(rewards:Map < String,i64 >)->Result < Map<String , i64>,String >{return result::ok(rewards)}

fn other(){return 1}
";
    let edits = range_format_source(
        source,
        DiagnosticRange::new(Position::new(0, 0), Position::new(1, 0)),
    );
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(
        formatted,
        "\
fn load_rewards(rewards: Map<String, i64>) -> Result<Map<String, i64>, String> {
    return result::ok(rewards)
}

fn other(){return 1}
"
    );
}

#[test]
fn on_type_formatting_compacts_builtin_container_type_arguments() {
    let source = "\
fn load_rewards(rewards:Map < String,i64 >)->Result < Map<String , i64>,String >{return result::ok(rewards)}

fn other(){return 1}
";
    let line_index = LineIndex::new(source);
    let close = source.find('}').expect("completed function body") + 1;
    let edits = on_type_format_source(source, line_index.position(close), "}");
    let formatted = apply_range_edits(source, &edits);

    assert_eq!(
        formatted,
        "\
fn load_rewards(rewards: Map<String, i64>) -> Result<Map<String, i64>, String> {
    return result::ok(rewards)
}

fn other(){return 1}
"
    );
}
