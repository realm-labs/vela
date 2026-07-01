use super::*;
use vela_common::HostTypeId;
use vela_def::{DefPath, FieldId};
use vela_host::target::HostPathPart;

#[test]
fn semantic_function_defaults_are_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn grant(base, amount = 10, bonus = amount + 1) {
    return base + amount + bonus;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("grant").expect("grant function");
    assert_cst_body(
        &payload.body,
        source,
        "{\n    return base + amount + bonus;\n}",
    );
    assert_cst_statements(
        &payload.body,
        &[(SyntaxStatementKind::Return, "return base + amount + bonus;")],
    );
    assert!(payload.param_defaults[0].is_none());
    assert_cst_param_default(&payload.param_defaults[1], source, "10");
    assert_cst_param_default(&payload.param_defaults[2], source, "amount + 1");
    let bonus_default = payload.param_defaults[2]
        .as_ref()
        .expect("bonus default payload");
    assert_eq!(
        bonus_default.expression.syntax().text().to_string(),
        "amount + 1",
    );

    compile_program_source(source, text).expect("CST-backed defaults should compile");
}

#[test]
fn compiler_lowers_let_block_parameter_defaults_from_cst() {
    compile_program_source(
        SourceId::new(1),
        r#"
fn grant(amount = { let base = 10; let bonus = base + 2; bonus }) {
    return amount;
}

fn main() {
    return grant();
}
"#,
    )
    .expect("CST-backed let block defaults should compile");
}

#[test]
fn compiler_lowers_typed_let_block_parameter_defaults_from_cst() {
    compile_program_source(
        SourceId::new(1),
        r#"
fn grant(amount = { let base: i8 = 10; let bonus: i8 = base + 2; bonus }) {
    return amount;
}

fn main() {
    return grant();
}
"#,
    )
    .expect("CST-backed typed let block defaults should compile");
}

#[test]
fn compiler_lowers_path_call_parameter_defaults_from_cst() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn helper(lhs, rhs) {
    return lhs + rhs;
}

fn grant(amount = helper(rhs = 2, lhs = 1)) {
    return amount;
}

fn main() {
    return grant();
}
"#,
    )
    .expect("CST-backed path call default should compile");
    let grant = program.function("grant").expect("grant function");

    assert!(grant.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::CallFunction { name, args, .. }
            if name == "helper" && args.len() == 2
    )));
}

#[test]
fn compiler_lowers_record_parameter_defaults_from_cst() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    amount: i64
    label: String = "xp"
}

fn grant(value, reward = Reward { amount: value }) {
    return reward;
}

fn main() {
    return grant(7);
}
"#,
    )
    .expect("CST-backed record default should compile");
    let grant = program.function("grant").expect("grant function");

    assert!(grant.instructions.iter().any(|instruction| {
        matches!(
            &instruction.kind,
            UnlinkedInstructionKind::MakeRecord { type_name, fields, .. }
                if type_name == "Reward"
                    && fields.iter().any(|(name, _)| name == "amount")
                    && fields.iter().any(|(name, _)| name == "label")
        )
    }));
    assert!(
        grant.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GuardType { .. }
        )),
        "dynamic record default field should keep schema type guard"
    );
}

#[test]
fn compiler_lowers_record_field_parameter_defaults_from_cst() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    amount: i64
    label: String = "xp"
}

fn grant(value = Reward { amount: 7 }.amount) {
    return value;
}

fn main() {
    return grant();
}
"#,
    )
    .expect("CST-backed record field default should compile");
    let grant = program.function("grant").expect("grant function");

    assert!(grant.instructions.iter().any(|instruction| {
        matches!(
            &instruction.kind,
            UnlinkedInstructionKind::GetRecordSlot { field, .. } if field == "amount"
        )
    }));
}

#[test]
fn compiler_lowers_call_receiver_field_parameter_defaults_from_cst() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    amount: i64
}

fn helper() {
    return Reward { amount: 7 };
}

fn grant(value = helper().amount) {
    return value;
}

fn main() {
    return grant();
}
"#,
    )
    .expect("CST-backed call receiver field default should compile");
    let grant = program.function("grant").expect("grant function");

    assert!(grant.instructions.iter().any(|instruction| {
        matches!(
            &instruction.kind,
            UnlinkedInstructionKind::GetRecordField { field, .. } if field == "amount"
        )
    }));
}

#[test]
fn compiler_lowers_match_parameter_defaults_from_cst() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
enum RewardKind {
    Small
    Large
}

fn grant(kind, amount = match kind {
    RewardKind::Small => 1
    RewardKind::Large => 2
    _ => 0
}) {
    return amount;
}

fn main() {
    return grant(null);
}
"#,
    )
    .expect("CST-backed match default should compile");
    let grant = program.function("grant").expect("grant function");

    assert!(grant.instructions.iter().any(|instruction| {
        matches!(
            &instruction.kind,
            UnlinkedInstructionKind::EnumTagEqual { enum_name, variant, .. }
                if enum_name == "RewardKind" && variant == "Small"
        )
    }));
}

#[test]
fn compiler_lowers_binding_match_parameter_defaults_from_cst() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn pick(value, copy = match value {
    bound if bound > 0 => bound
    _ => 0
}) {
    return copy;
}

fn main() {
    return pick(7);
}
"#,
    )
    .expect("CST-backed binding match default should compile");
    let pick = program.function("pick").expect("pick function");

    assert!(
        pick.frame
            .slot("bound", crate::FrameSlotKind::PatternBinding)
            .is_some()
    );
}

#[test]
fn compiler_reports_typed_let_block_parameter_default_mismatches_from_cst() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
fn grant(amount = { let base: bool = 10; base }) {
    return amount;
}
"#,
    )
    .expect_err("CST-backed typed let block default should reject mismatched literals");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::type_contract_mismatch"]
    );
}

#[test]
fn semantic_script_method_defaults_are_cst_payloads() {
    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
struct Counter { value: i64 }
impl Counter {
    fn add(self, amount = 1) {
        self.value += amount;
    }
}
"#,
    )
    .expect("source should parse");
    let methods = semantic.script_impl_methods();
    let method = methods
        .iter()
        .find(|method| method.method_name == "add")
        .expect("script method");
    assert_cst_body(
        &method.body,
        source,
        "{\n        self.value += amount;\n    }",
    );
    assert_cst_statements(
        &method.body,
        &[(SyntaxStatementKind::Expr, "self.value += amount;")],
    );
    assert_cst_expr_statements(
        &method.body,
        &[(SyntaxExpressionKind::Assign, "self.value += amount")],
    );
    assert!(method.default_values[0].is_none());
    assert_cst_param_default(&method.default_values[1], source, "1");
    let amount_default = method.default_values[1]
        .as_ref()
        .expect("amount default payload");
    assert_eq!(amount_default.expression.syntax().text().to_string(), "1",);
}

#[test]
fn compiler_lowers_host_field_parameter_defaults_from_cst() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
                .host_runtime_id(77),
        )
        .expect("host type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "Player", "level"),
                player,
            )
            .host_runtime_id(3)
            .type_hint(Some("i64".to_owned())),
        )
        .expect("host field should register");

    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn grant(player: Player, value = player.level) {
    return value;
}

fn main(player: Player) {
    return grant(player);
}
"#,
        registry.compile_view(),
    )
    .expect("CST-backed host field parameter default should compile");
    let grant = program.function("grant").expect("grant function");
    let target = grant
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            UnlinkedInstructionKind::HostRead { target, .. } => Some(target),
            _ => None,
        })
        .expect("parameter default should emit a host read");
    let plan = grant.host_target(target).expect("host target should exist");

    assert_eq!(plan.root_type, HostTypeId::new(77));
    assert_eq!(
        plan.parts.as_slice(),
        [HostPathPart::Field(FieldId::new(3))]
    );
}

#[test]
fn unsupported_parameter_defaults_report_from_cst_without_fallback() {
    let error = compile_function_source(
        SourceId::new(1),
        r#"
fn main(value = |item| item) {
    return value;
}
"#,
        "main",
    )
    .expect_err("lambda parameter defaults are not supported");

    assert_eq!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("parameter default expression")
    );
}
