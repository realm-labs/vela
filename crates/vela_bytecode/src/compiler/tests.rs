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

fn assert_cst_for_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .filter_map(|statement| {
            let body = statement.for_body_payload()?;
            let statements = body.statement_payloads();
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
            .map(|body| {
                body.iter()
                    .map(|(kind, text)| (*kind, (*text).to_owned()))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    );
}

fn assert_cst_if_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_then: &[Vec<(SyntaxStatementKind, &str)>],
    expected_else: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let payloads = statements
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::if_payload)
        .collect::<Vec<_>>();
    let then_actual = payloads
        .iter()
        .filter_map(body_payloads::CompilerIfPayload::then_body)
        .map(cst_statement_texts)
        .collect::<Vec<_>>();
    let else_actual = payloads
        .iter()
        .filter_map(body_payloads::CompilerIfPayload::else_body)
        .map(cst_statement_texts)
        .collect::<Vec<_>>();
    assert_eq!(then_actual, expected_statement_texts(expected_then));
    assert_eq!(else_actual, expected_statement_texts(expected_else));
}

fn assert_cst_statement_else_if_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_then: &[Vec<(SyntaxStatementKind, &str)>],
    expected_else: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let nested = statements
        .iter()
        .filter_map(body_payloads::CompilerStatementPayload::if_payload)
        .filter_map(|payload| {
            let nested = payload.else_if()?;
            Some((
                nested.then_body().map(cst_statement_texts),
                nested.else_body().map(cst_statement_texts),
            ))
        })
        .collect::<Vec<_>>();
    let then_actual = nested
        .iter()
        .filter_map(|(then, _)| then.clone())
        .collect::<Vec<_>>();
    let else_actual = nested
        .iter()
        .filter_map(|(_, else_body)| else_body.clone())
        .collect::<Vec<_>>();
    assert_eq!(then_actual, expected_statement_texts(expected_then));
    assert_eq!(else_actual, expected_statement_texts(expected_else));
}

fn assert_cst_let_initializer_if_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_then: &[Vec<(SyntaxStatementKind, &str)>],
    expected_else: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let payloads = statements
        .iter()
        .filter_map(|statement| statement.let_initializer_if_payload())
        .collect::<Vec<_>>();
    let then_actual = payloads
        .iter()
        .filter_map(body_payloads::CompilerIfPayload::then_body)
        .map(cst_statement_texts)
        .collect::<Vec<_>>();
    let else_actual = payloads
        .iter()
        .filter_map(body_payloads::CompilerIfPayload::else_body)
        .map(cst_statement_texts)
        .collect::<Vec<_>>();
    assert_eq!(then_actual, expected_statement_texts(expected_then));
    assert_eq!(else_actual, expected_statement_texts(expected_else));
}

fn assert_cst_return_value_if_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_then: &[Vec<(SyntaxStatementKind, &str)>],
    expected_else: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let payloads = statements
        .iter()
        .filter_map(|statement| statement.return_value_if_payload())
        .collect::<Vec<_>>();
    let then_actual = payloads
        .iter()
        .filter_map(body_payloads::CompilerIfPayload::then_body)
        .map(cst_statement_texts)
        .collect::<Vec<_>>();
    let else_actual = payloads
        .iter()
        .filter_map(body_payloads::CompilerIfPayload::else_body)
        .map(cst_statement_texts)
        .collect::<Vec<_>>();
    assert_eq!(then_actual, expected_statement_texts(expected_then));
    assert_eq!(else_actual, expected_statement_texts(expected_else));
}

fn assert_cst_let_initializer_else_if_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_then: &[Vec<(SyntaxStatementKind, &str)>],
    expected_else: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let nested = statements
        .iter()
        .filter_map(|statement| statement.let_initializer_if_payload())
        .filter_map(|payload| {
            let nested = payload.else_if()?;
            Some((
                nested.then_body().map(cst_statement_texts),
                nested.else_body().map(cst_statement_texts),
            ))
        })
        .collect::<Vec<_>>();
    let then_actual = nested
        .iter()
        .filter_map(|(then, _)| then.clone())
        .collect::<Vec<_>>();
    let else_actual = nested
        .iter()
        .filter_map(|(_, else_body)| else_body.clone())
        .collect::<Vec<_>>();
    assert_eq!(then_actual, expected_statement_texts(expected_then));
    assert_eq!(else_actual, expected_statement_texts(expected_else));
}

fn assert_cst_return_value_else_if_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_then: &[Vec<(SyntaxStatementKind, &str)>],
    expected_else: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let nested = statements
        .iter()
        .filter_map(|statement| statement.return_value_if_payload())
        .filter_map(|payload| {
            let nested = payload.else_if()?;
            Some((
                nested.then_body().map(cst_statement_texts),
                nested.else_body().map(cst_statement_texts),
            ))
        })
        .collect::<Vec<_>>();
    let then_actual = nested
        .iter()
        .filter_map(|(then, _)| then.clone())
        .collect::<Vec<_>>();
    let else_actual = nested
        .iter()
        .filter_map(|(_, else_body)| else_body.clone())
        .collect::<Vec<_>>();
    assert_eq!(then_actual, expected_statement_texts(expected_then));
    assert_eq!(else_actual, expected_statement_texts(expected_else));
}

fn assert_cst_match_arm_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .flat_map(|statement| statement.match_arm_payloads().unwrap_or_default())
        .filter_map(|arm| {
            let _syntax_arm = arm.syntax_arm()?;
            let body = arm.body_block_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_let_initializer_match_arm_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .flat_map(|statement| {
            statement
                .let_initializer_match_arm_payloads()
                .unwrap_or_default()
        })
        .filter_map(|arm| {
            let _syntax_arm = arm.syntax_arm()?;
            let body = arm.body_block_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_return_value_match_arm_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .flat_map(|statement| {
            statement
                .return_value_match_arm_payloads()
                .unwrap_or_default()
        })
        .filter_map(|arm| {
            let _syntax_arm = arm.syntax_arm()?;
            let body = arm.body_block_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_let_initializer_block_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .filter_map(|statement| statement.let_initializer_block_body_payload())
        .map(|body| cst_statement_texts(&body))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_return_value_block_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .filter_map(|statement| statement.return_value_block_body_payload())
        .map(|body| cst_statement_texts(&body))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn assert_cst_let_initializer_block_tail_if_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected_then: &[Vec<(SyntaxStatementKind, &str)>],
    expected_else: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let payloads = statements
        .iter()
        .filter_map(|statement| statement.let_initializer_block_body_payload())
        .flat_map(|block| block.statement_payloads())
        .filter_map(|statement| statement.expression_if_payload())
        .collect::<Vec<_>>();
    let then_actual = payloads
        .iter()
        .filter_map(body_payloads::CompilerIfPayload::then_body)
        .map(cst_statement_texts)
        .collect::<Vec<_>>();
    let else_actual = payloads
        .iter()
        .filter_map(body_payloads::CompilerIfPayload::else_body)
        .map(cst_statement_texts)
        .collect::<Vec<_>>();
    assert_eq!(then_actual, expected_statement_texts(expected_then));
    assert_eq!(else_actual, expected_statement_texts(expected_else));
}

fn assert_cst_let_initializer_block_tail_match_arm_body_payloads(
    body: &body_payloads::CompilerBodyPayload<'_>,
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) {
    let statements = body.statement_payloads();
    let actual = statements
        .iter()
        .filter_map(|statement| statement.let_initializer_block_body_payload())
        .flat_map(|block| block.statement_payloads())
        .flat_map(|statement| {
            statement
                .expression_match_arm_payloads()
                .unwrap_or_default()
        })
        .filter_map(|arm| {
            let _syntax_arm = arm.syntax_arm()?;
            let body = arm.body_block_payload()?;
            Some(cst_statement_texts(&body))
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_statement_texts(expected));
}

fn cst_statement_texts(
    body: &body_payloads::CompilerBodyPayload<'_>,
) -> Vec<(SyntaxStatementKind, String)> {
    body.statement_payloads()
        .iter()
        .filter_map(|statement| {
            let syntax = statement.syntax_statement()?;
            Some((syntax.statement_kind(), syntax.syntax().text().to_string()))
        })
        .collect()
}

fn expected_statement_texts(
    expected: &[Vec<(SyntaxStatementKind, &str)>],
) -> Vec<Vec<(SyntaxStatementKind, String)>> {
    expected
        .iter()
        .map(|body| {
            body.iter()
                .map(|(kind, text)| (*kind, (*text).to_owned()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
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
mod cst_payloads;
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
