use super::*;
use crate::verification::VerificationErrorKind;
use crate::{
    CacheSiteKind, CallArgument, Register, UnlinkedCodeObject, UnlinkedInstruction,
    UnlinkedInstructionKind, UnlinkedProgram,
};
use vela_def::{DefPath, FunctionId, MethodId};
use vela_syntax::ast::{AstNode, BinaryOp, SyntaxExpressionKind, SyntaxStatementKind};

fn assert_cst_param_default(
    default: &Option<ParamDefaultValue>,
    expected_source: SourceId,
    expected_text: &str,
) {
    let Some(ParamDefaultValue::Syntax {
        source,
        expression,
        fallback: _,
    }) = default
    else {
        panic!("expected CST-backed parameter default");
    };
    assert_eq!(*source, expected_source);
    assert_eq!(expression.syntax().text().to_string(), expected_text);
}

fn assert_cst_body(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_source: SourceId,
    expected_text: &str,
) {
    let payload = body.syntax_payload().expect("expected CST-backed body");
    assert_eq!(payload.source, expected_source);
    assert_eq!(payload.body.syntax().text().to_string(), expected_text);
}

fn assert_cst_statements(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[(SyntaxStatementKind, &str)],
) {
    let statements = body.statement_payloads();
    assert_eq!(statements.len(), expected.len());
    for (statement, (expected_kind, expected_text)) in statements.iter().zip(expected) {
        let syntax = statement
            .syntax_statement()
            .expect("expected CST-backed statement");
        assert_eq!(syntax.statement_kind(), *expected_kind);
        assert_eq!(syntax.syntax().text().to_string(), *expected_text);
    }
}

fn assert_cst_expr_statements(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[(SyntaxExpressionKind, &str)],
) {
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .filter_map(|statement| {
            let syntax = statement.syntax_statement()?;
            let expr = syntax.as_expr()?.expression()?;
            Some((expr.expression_kind(), expr.syntax().text().to_string()))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|(kind, text)| (*kind, (*text).to_owned()))
            .collect::<Vec<_>>()
    );
}

fn assert_cst_block_statement_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .filter_map(|statement| {
            let block = statement.block_body_payload()?;
            let statements = block.statement_payloads();
            Some(
                statements
                    .iter()
                    .filter_map(|statement| {
                        let syntax = statement.syntax_statement()?;
                        Some((syntax.statement_kind(), syntax.syntax().text().to_string()))
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|block| {
                block
                    .iter()
                    .map(|(kind, text)| (*kind, (*text).to_owned()))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    );
}

fn assert_cst_let_initializers(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[(SyntaxExpressionKind, &str)],
) {
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .filter_map(|statement| {
            let syntax = statement.syntax_statement()?;
            let initializer = syntax.as_let()?.initializer()?;
            Some((
                initializer.expression_kind(),
                initializer.syntax().text().to_string(),
            ))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|(kind, text)| (*kind, (*text).to_owned()))
            .collect::<Vec<_>>()
    );
}

fn assert_cst_return_values(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[(SyntaxExpressionKind, &str)],
) {
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .filter_map(|statement| {
            let syntax = statement.syntax_statement()?;
            let value = syntax.as_return()?.expression()?;
            Some((value.expression_kind(), value.syntax().text().to_string()))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|(kind, text)| (*kind, (*text).to_owned()))
            .collect::<Vec<_>>()
    );
}

fn assert_cst_for_iterables(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[(SyntaxExpressionKind, Option<BinaryOp>, &str)],
) {
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .filter_map(|statement| {
            let syntax = statement.syntax_statement()?;
            let iterable = syntax.as_for()?.iterable()?;
            Some((
                iterable.expression_kind(),
                iterable.as_binary().and_then(|binary| binary.operator()),
                iterable.syntax().text().to_string(),
            ))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|(kind, op, text)| (*kind, *op, (*text).to_owned()))
            .collect::<Vec<_>>()
    );
}

fn assert_cst_if_conditions(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[(SyntaxExpressionKind, Option<BinaryOp>, &str)],
) {
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .filter_map(|statement| {
            let syntax = statement.syntax_statement()?;
            let condition = syntax.as_if()?.condition()?;
            Some((
                condition.expression_kind(),
                condition.as_binary().and_then(|binary| binary.operator()),
                condition.syntax().text().to_string(),
            ))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|(kind, op, text)| (*kind, *op, (*text).to_owned()))
            .collect::<Vec<_>>()
    );
}

fn semantic_diagnostic_codes(error: CompileError) -> Vec<String> {
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    diagnostics
        .into_iter()
        .filter_map(|diagnostic| diagnostic.code)
        .collect()
}

fn stable_test_trait_method_id(trait_name: &str, method_name: &str) -> MethodId {
    MethodId::new(u128::from(vela_common::stable_id(
        "trait_method",
        trait_name,
        method_name,
    )))
}

fn stable_test_inherent_method_id(type_name: &str, method_name: &str) -> MethodId {
    MethodId::new(u128::from(vela_common::stable_id(
        "inherent_method",
        type_name,
        method_name,
    )))
}

#[test]
fn semantic_function_defaults_are_cst_payloads() {
    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn grant(base, amount = 10, bonus = amount + 1) {
    return base + amount + bonus;
}
"#,
    )
    .expect("source should parse");
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
}

#[test]
fn semantic_function_assignment_statement_expression_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn assign() {
    let total = 1;
    total += 2;
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("assign").expect("assign function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let total = 1;"),
            (SyntaxStatementKind::Expr, "total += 2;"),
            (SyntaxStatementKind::Return, "return total;"),
        ],
    );
    assert_cst_expr_statements(
        &payload.body,
        &[(SyntaxExpressionKind::Assign, "total += 2")],
    );

    compile_program_source(source, text).expect("CST-backed assignment body should compile");
}

#[test]
fn semantic_function_let_initializer_expression_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn choose() {
    let total = if true {
        1
    } else {
        2
    };
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("choose").expect("choose function");
    assert_cst_statements(
        &payload.body,
        &[
            (
                SyntaxStatementKind::Let,
                "let total = if true {\n        1\n    } else {\n        2\n    };",
            ),
            (SyntaxStatementKind::Return, "return total;"),
        ],
    );
    assert_cst_let_initializers(
        &payload.body,
        &[(
            SyntaxExpressionKind::If,
            "if true {\n        1\n    } else {\n        2\n    }",
        )],
    );

    compile_program_source(source, text).expect("CST-backed let initializer body should compile");
}

#[test]
fn semantic_function_return_value_expression_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn choose() {
    return if true {
        1
    } else {
        2
    };
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("choose").expect("choose function");
    assert_cst_statements(
        &payload.body,
        &[(
            SyntaxStatementKind::Return,
            "return if true {\n        1\n    } else {\n        2\n    };",
        )],
    );
    assert_cst_return_values(
        &payload.body,
        &[(
            SyntaxExpressionKind::If,
            "if true {\n        1\n    } else {\n        2\n    }",
        )],
    );

    compile_program_source(source, text).expect("CST-backed return value body should compile");
}

#[test]
fn semantic_function_for_iterable_expression_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn sum() {
    let total = 0;
    for value in 0..3 {
        total += value;
    }
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("sum").expect("sum function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let total = 0;"),
            (
                SyntaxStatementKind::For,
                "for value in 0..3 {\n        total += value;\n    }",
            ),
            (SyntaxStatementKind::Return, "return total;"),
        ],
    );
    assert_cst_for_iterables(
        &payload.body,
        &[(SyntaxExpressionKind::Binary, Some(BinaryOp::Range), "0..3")],
    );

    let program =
        compile_program_source(source, text).expect("CST-backed range for body should compile");
    let function = program.function("sum").expect("sum bytecode");
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::I64RangeNext { .. }
    )));
}

#[test]
fn semantic_function_if_condition_expression_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn check() {
    let value: i64 = 10;
    if value > 5 {
        return 1;
    }
    return 0;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("check").expect("check function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let value: i64 = 10;"),
            (
                SyntaxStatementKind::If,
                "if value > 5 {\n        return 1;\n    }",
            ),
            (SyntaxStatementKind::Return, "return 0;"),
        ],
    );
    assert_cst_if_conditions(
        &payload.body,
        &[(
            SyntaxExpressionKind::Binary,
            Some(BinaryOp::Greater),
            "value > 5",
        )],
    );

    let program =
        compile_program_source(source, text).expect("CST-backed if condition should compile");
    let function = program.function("check").expect("check bytecode");
    assert!(function.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
            op: crate::I64CompareOp::Greater,
            imm: 5,
            ..
        }
    )));
}

#[test]
fn semantic_function_block_statement_body_is_cst_payload() {
    let source = SourceId::new(1);
    let text = r#"
fn scoped() {
    let total = 0;
    {
        total += 1;
    }
    return total;
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("scoped").expect("scoped function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let total = 0;"),
            (SyntaxStatementKind::Block, "{\n        total += 1;\n    }"),
            (SyntaxStatementKind::Return, "return total;"),
        ],
    );
    assert_cst_block_statement_payloads(
        &payload.body,
        &[vec![(SyntaxStatementKind::Expr, "total += 1;")]],
    );

    compile_program_source(source, text).expect("CST-backed block statement body should compile");
}

#[test]
fn semantic_function_control_flow_statements_are_cst_payloads() {
    let source = SourceId::new(1);
    let text = r#"
fn flow() {
    let total = 0;
    if total == 0 {
        return 1;
    }
    match total {
        0 => { return 0; },
        _ => { return total; },
    }
}
"#;
    let semantic = parse_semantic_source(source, text).expect("source should parse");
    let (payload, _, _) = semantic.function("flow").expect("flow function");
    assert_cst_statements(
        &payload.body,
        &[
            (SyntaxStatementKind::Let, "let total = 0;"),
            (
                SyntaxStatementKind::If,
                "if total == 0 {\n        return 1;\n    }",
            ),
            (
                SyntaxStatementKind::Match,
                "match total {\n        0 => { return 0; },\n        _ => { return total; },\n    }",
            ),
        ],
    );

    compile_program_source(source, text).expect("CST-backed control-flow body should compile");
}

#[test]
fn compiler_entry_points_return_unlinked_bytecode() {
    let program: UnlinkedProgram = compile_program_source(
        SourceId::new(1),
        r#"
fn main() {
    return 42;
}
"#,
    )
    .expect("program should compile");
    assert!(program.function("main").is_some());

    let code: UnlinkedCodeObject = compile_function_source(
        SourceId::new(2),
        r#"
fn main() {
    return 7;
}
"#,
        "main",
    )
    .expect("function should compile");
    assert_eq!(code.name, "main");
}

#[test]
fn compiler_boundary_rejects_invalid_program_bytecode() {
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));
    let mut program = UnlinkedProgram::new();
    program.insert_function(code);

    let error = verify_program(program).expect_err("invalid bytecode should fail verification");
    let CompileErrorKind::BytecodeVerification(error) = error.kind else {
        panic!("expected bytecode verification error");
    };
    assert_eq!(error.function, "main");
    assert_eq!(error.instruction, Some(0));
    assert_eq!(
        error.kind,
        VerificationErrorKind::RegisterOutOfBounds {
            register: Register(2),
            register_count: 1,
        }
    );
}

#[test]
fn compiler_boundary_rejects_invalid_function_bytecode() {
    let mut code = UnlinkedCodeObject::new("main", 1);
    code.push_instruction(UnlinkedInstruction::new(UnlinkedInstructionKind::Return {
        src: Register(2),
    }));

    let error = verify_code_object(code).expect_err("invalid bytecode should fail verification");
    let CompileErrorKind::BytecodeVerification(error) = error.kind else {
        panic!("expected bytecode verification error");
    };
    assert_eq!(error.function, "main");
    assert_eq!(error.instruction, Some(0));
}

#[test]
fn compiler_records_cache_site_metadata_for_cacheable_instructions() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), "Player"))
                .host_runtime_id(77),
        )
        .expect("Player type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "Player", "level"),
                player,
            )
            .host_runtime_id(1),
        )
        .expect("Player::level field should register");
    registry
        .register_function(
            vela_registry::FunctionDef::new(
                DefPath::function("host", std::iter::empty::<&str>(), "give_reward"),
                vela_registry::FunctionSignature::new(
                    [vela_registry::ParamDef::new("amount", Some("i64"))],
                    None::<vela_registry::TypeHintDef>,
                ),
            )
            .with_id(FunctionId::new(7)),
        )
        .expect("give_reward function should register");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
global bonus: i64;

struct Reward {
    gold: i64,
}

impl Reward {
    fn score(self, amount) {
        return self.gold + amount;
    }
}

fn main(player: Player) {
    let reward = Reward { gold: bonus };
    let current = player.level;
    player.level = current + reward.gold;
    give_reward(reward.score(1));
    return player.level;
}
"#,
        registry.compile_view(),
    )
    .expect("program should compile");
    let main = program.function("main").expect("main should exist");
    let site_kinds = main
        .cache_sites
        .sites()
        .iter()
        .map(|site| site.kind)
        .collect::<Vec<_>>();

    assert!(site_kinds.contains(&CacheSiteKind::GlobalRead));
    assert!(site_kinds.contains(&CacheSiteKind::NativeCall));
    assert!(site_kinds.contains(&CacheSiteKind::MethodCall));
    assert!(site_kinds.contains(&CacheSiteKind::RecordFieldRead));
    assert!(site_kinds.contains(&CacheSiteKind::HostPathRead));
    assert!(site_kinds.contains(&CacheSiteKind::HostPathWrite));
    let load_global_site = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::LoadGlobal { cache_site, .. } => *cache_site,
            _ => None,
        })
        .expect("load global should carry cache site");
    assert_eq!(
        main.cache_sites
            .get(load_global_site)
            .expect("load global cache site should exist")
            .kind,
        CacheSiteKind::GlobalRead
    );
    let native_call_site = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallNative { cache_site, .. } => *cache_site,
            _ => None,
        })
        .expect("native call should carry cache site");
    assert_eq!(
        main.cache_sites
            .get(native_call_site)
            .expect("native call cache site should exist")
            .kind,
        CacheSiteKind::NativeCall
    );
    for (index, site) in main.cache_sites.sites().iter().enumerate() {
        assert_eq!(site.id.index(), index);
        assert_eq!(site.function, "main");
        assert!(main.instructions.get(site.instruction_offset.0).is_some());
    }
}

mod call_diagnostics;
mod closures_and_bindings;
mod diagnostics;
mod expressions;
mod host_paths;
mod literals_and_calls;
mod loops_and_errors;
mod module_resolution;
mod script_methods;
mod type_contract_constructors;
mod type_contracts;
mod value_method_shapes;
