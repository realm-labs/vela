use super::*;
use vela_common::SourceId;
use vela_syntax::parser::parse_source;

fn classify(text: &str, needle: &str) -> CursorContext {
    let offset = text.find(needle).expect("needle should exist") + needle.len();
    let parsed = parse_source(SourceId::new(1), text);
    cursor_context_at(text, LineIndex::new(text).position(offset), Some(&parsed))
}

fn classify_offset(text: &str, offset: usize) -> CursorContext {
    let parsed = parse_source(SourceId::new(1), text);
    cursor_context_at(text, LineIndex::new(text).position(offset), Some(&parsed))
}

#[test]
fn cursor_context_classifies_item_boundary_keywords() {
    let cursor = classify("f", "f");

    assert_eq!(cursor.kind(), CursorContextKind::Item);
    assert_eq!(cursor.prefix(), "f");
}

#[test]
fn cursor_context_classifies_type_hints() {
    let cursor = classify("pub fn main(player: Pl) { return 1 }", "Pl");

    assert_eq!(cursor.kind(), CursorContextKind::Type);
}

#[test]
fn cursor_context_classifies_use_import_context() {
    let cursor = classify("use re", "re");

    assert_eq!(cursor.kind(), CursorContextKind::UseImport);
}

#[test]
fn cursor_context_classifies_member_access() {
    let cursor = classify("pub fn main(player) { player.le }", "le");

    assert_eq!(cursor.kind(), CursorContextKind::MemberAccess);
    assert_eq!(cursor.member_receiver(), Some(TextRange::new(22, 28)));
}

#[test]
fn cursor_context_uses_syntax_receiver_span_for_member_access() {
    let text = "pub fn main() { current_player().le }";
    let cursor = classify(text, "le");
    let receiver = cursor.member_receiver().expect("receiver range");

    assert_eq!(cursor.kind(), CursorContextKind::MemberAccess);
    assert_eq!(&text[receiver.start..receiver.end], "current_player()");
}

#[test]
fn cursor_context_classifies_module_path() {
    let cursor = classify("use game::r", "r");

    assert_eq!(cursor.kind(), CursorContextKind::ModulePath);
    assert_eq!(cursor.module_base(), Some("game"));
    assert_eq!(cursor.module_path_role(), ModulePathRole::Expression);
}

#[test]
fn cursor_context_marks_type_module_path_role() {
    let cursor = classify("pub fn main(item: game::reward::Re) { }", "Re");

    assert_eq!(cursor.kind(), CursorContextKind::ModulePath);
    assert_eq!(cursor.module_base(), Some("game::reward"));
    assert_eq!(cursor.module_path_role(), ModulePathRole::Type);
}

#[test]
fn cursor_context_classifies_record_expression_fields() {
    let text = "pub struct Player { level: i64 }\npub fn main() { let player = Player { xp } }";
    let cursor = classify(text, "xp");

    assert_eq!(cursor.kind(), CursorContextKind::RecordExpressionField);
}

#[test]
fn cursor_context_classifies_record_type_fields() {
    let cursor = classify("pub struct Player { le }", "le");

    assert_eq!(cursor.kind(), CursorContextKind::RecordTypeField);
}

#[test]
fn cursor_context_classifies_enum_record_variant_fields() {
    let cursor = classify("pub enum Quest { Reward { am } }", "am");

    assert_eq!(cursor.kind(), CursorContextKind::RecordTypeField);
}

#[test]
fn cursor_context_keeps_record_type_field_type_hints_as_type_context() {
    let cursor = classify("pub struct Player { level: Score }", "Score");

    assert_eq!(cursor.kind(), CursorContextKind::Type);
}

#[test]
fn cursor_context_classifies_map_keys() {
    let text = "pub fn main() { let rewards: Map<QuestState, i64> = { Co: 2 } }";
    let cursor = classify(text, "Co");

    assert_eq!(cursor.kind(), CursorContextKind::MapKey);
}

#[test]
fn cursor_context_classifies_call_arguments() {
    let text = "pub fn main() { grant(am) }";
    let cursor = classify(text, "am");
    let callee = cursor.call_callee().expect("callee range");

    assert_eq!(cursor.kind(), CursorContextKind::CallArgument);
    assert_eq!(&text[callee.start..callee.end], "grant");
}

#[test]
fn cursor_context_uses_inner_syntax_callee_for_nested_call_arguments() {
    let text = "pub fn main() { outer(inner(am)) }";
    let cursor = classify(text, "am");
    let callee = cursor.call_callee().expect("callee range");

    assert_eq!(cursor.kind(), CursorContextKind::CallArgument);
    assert_eq!(&text[callee.start..callee.end], "inner");
}

#[test]
fn cursor_context_classifies_lambda_parameters() {
    let text = "pub fn main(items) { items.map(|it| it) }";
    let cursor = classify(text, "|");
    let receiver = cursor.member_receiver().expect("receiver range");
    let method = cursor.lambda_method().expect("method range");

    assert_eq!(cursor.kind(), CursorContextKind::LambdaParameter);
    assert_eq!(cursor.prefix(), "");
    assert_eq!(&text[receiver.start..receiver.end], "items");
    assert_eq!(&text[method.start..method.end], "map");
}

#[test]
fn cursor_context_classifies_for_loop_patterns() {
    let cursor = classify("pub fn main(items) { for re in items { } }", "re");

    assert_eq!(cursor.kind(), CursorContextKind::Pattern);
}

#[test]
fn cursor_context_classifies_indexed_for_loop_patterns() {
    let cursor = classify(
        "pub fn main(items) { for index, Reward::Grant { amount } in items { } }",
        "Reward::Grant",
    );

    assert_eq!(cursor.kind(), CursorContextKind::Pattern);
}

#[test]
fn cursor_context_classifies_match_arm_patterns() {
    let cursor = classify(
        "pub fn main(state) { match state { Quest::Active { amount } => amount } }",
        "Active",
    );

    assert_eq!(cursor.kind(), CursorContextKind::Pattern);
}

#[test]
fn cursor_context_classifies_record_pattern_fields() {
    let cursor = classify(
        "pub fn main(state) { match state { Quest::Active { amount } => amount } }",
        "amount",
    );

    assert_eq!(cursor.kind(), CursorContextKind::Pattern);
}

#[test]
fn cursor_context_keeps_match_arm_body_as_expression() {
    let text = "pub fn main(state) { match state { Quest::Active { amount } => amount } }";
    let offset = text.rfind("amount").expect("body expression") + "amount".len();
    let cursor = classify_offset(text, offset);

    assert_eq!(cursor.kind(), CursorContextKind::Expression);
}

#[test]
fn cursor_context_classifies_statement_boundary() {
    let text = "pub fn main() { return 1 }";
    let cursor = classify_offset(text, text.find("return").expect("return should exist"));

    assert_eq!(cursor.kind(), CursorContextKind::Statement);
    assert_eq!(cursor.prefix(), "");
}

#[test]
fn cursor_context_classifies_expression_position() {
    let cursor = classify("pub fn main() { Pla }", "Pla");

    assert_eq!(cursor.kind(), CursorContextKind::Expression);
}

#[test]
fn cursor_context_recovers_useful_roles_in_incomplete_source() {
    let member_text = "pub fn main(player) { player.";
    let member_cursor = classify_offset(member_text, member_text.len());
    assert_eq!(member_cursor.kind(), CursorContextKind::MemberAccess);
    assert_eq!(
        member_cursor.member_receiver(),
        Some(TextRange::new(22, 28))
    );

    let type_cursor = classify("pub fn main(player: Pla", "Pla");
    assert_eq!(type_cursor.kind(), CursorContextKind::Type);

    let call_text = "pub fn main() { grant(";
    let call_cursor = classify_offset(call_text, call_text.len());
    assert_eq!(call_cursor.kind(), CursorContextKind::CallArgument);
    assert_eq!(call_cursor.call_callee(), Some(TextRange::new(16, 21)));
}
