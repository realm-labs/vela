use super::*;

pub(super) fn first_call_argument_payload_from_cst(
    source: SourceId,
    text: &str,
) -> body_payloads::CompilerArgumentPayload {
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, text);
    let cst_call = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST body")
        .statements()
        .next()
        .expect("CST statement")
        .as_expr()
        .expect("CST expression statement")
        .expression()
        .expect("CST expression");
    let payload =
        body_payloads::CompilerExpressionPayload::from_syntax(Some(source), Some(cst_call));
    payload
        .call_argument_payloads()
        .expect("CST call argument payloads")
        .into_iter()
        .next()
        .expect("CST call argument")
}

fn first_let_initializer_payload_from_cst(
    source: SourceId,
    text: &str,
) -> body_payloads::CompilerExpressionPayload<'static> {
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, text);
    let cst_initializer = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .next()
        .expect("CST let statement")
        .as_let()
        .expect("CST let")
        .initializer()
        .expect("CST initializer");
    body_payloads::CompilerExpressionPayload::from_syntax(Some(source), Some(cst_initializer))
}

pub(super) fn first_map_entry_payload_from_cst(
    source: SourceId,
    text: &str,
) -> body_payloads::CompilerMapEntryPayload {
    first_let_initializer_payload_from_cst(source, text)
        .map_entry_payloads()
        .expect("CST map entry payloads")
        .into_iter()
        .next()
        .expect("CST map entry")
}

pub(super) fn first_record_field_payload_from_cst(
    source: SourceId,
    text: &str,
) -> body_payloads::CompilerRecordFieldPayload {
    first_let_initializer_payload_from_cst(source, text)
        .record_field_payloads()
        .expect("CST record field payloads")
        .into_iter()
        .next()
        .expect("CST record field")
}

pub(super) fn first_return_match_arm_payload_from_cst(
    source: SourceId,
    text: &str,
) -> body_payloads::CompilerMatchArmPayload {
    let cst_parse = vela_syntax::parse::parse_source_with_id(source, text);
    let cst_match = cst_parse
        .tree()
        .functions()
        .next()
        .expect("CST function")
        .body()
        .expect("CST function body")
        .statements()
        .next()
        .expect("CST return statement")
        .as_return()
        .expect("CST return")
        .expression()
        .expect("CST return expression");
    let payload =
        body_payloads::CompilerExpressionPayload::from_syntax(Some(source), Some(cst_match));
    payload
        .match_arm_payloads()
        .expect("CST match arm payloads")
        .into_iter()
        .next()
        .expect("CST match arm")
}
