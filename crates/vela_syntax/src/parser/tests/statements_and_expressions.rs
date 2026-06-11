use super::*;

#[test]
fn parses_function_body_statements_and_expressions() {
    let parsed = parse_source(
        source_id(),
        r#"
fn on_kill(ctx, player, monster) {
    let rewards = [monster.exp, 2 + 3 * 4];
    player.exp += monster.exp;
    if player.exp >= ctx.config.exp_to_next_level(player.level) {
        player.level += 1;
        player.exp = 0;
    } else {
        return null;
    }
    for reward in rewards {
        player.inventory.add(reward.item_id, reward.count);
    }
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };
    assert_eq!(function.body.statements.len(), 4);
    assert!(matches!(
        function.body.statements[0].kind,
        StmtKind::Let { .. }
    ));
    assert!(matches!(
        function.body.statements[2].kind,
        StmtKind::Expr(Expr {
            kind: ExprKind::If(_),
            ..
        })
    ));
    assert!(matches!(
        function.body.statements[3].kind,
        StmtKind::For { .. }
    ));

    let StmtKind::Let {
        value: Some(value), ..
    } = &function.body.statements[0].kind
    else {
        panic!("expected initialized let");
    };
    let ExprKind::Array(items) = &value.kind else {
        panic!("expected array literal");
    };
    assert_eq!(items.len(), 2);
    assert!(matches!(
        items[1].kind,
        ExprKind::Binary {
            op: BinaryOp::Add,
            ..
        }
    ));
}

#[test]
fn parses_for_in_patterns() {
    let parsed = parse_source(
        source_id(),
        r#"
fn main(rewards) {
    for Reward::Grant { amount } in rewards {
        total += amount;
    }
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };
    let StmtKind::For { pattern, .. } = &function.body.statements[0].kind else {
        panic!("expected for statement");
    };
    let Pattern::RecordVariant { path, fields } = pattern else {
        panic!("expected record variant pattern");
    };
    assert_eq!(path, &["Reward", "Grant"]);
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "amount");
    assert!(fields[0].pattern.is_none());
}

#[test]
fn parses_indexed_for_in_patterns() {
    let parsed = parse_source(
        source_id(),
        r#"
fn main(rewards) {
    for index, Reward::Grant { amount } in rewards {
        total += index + amount;
    }
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };
    let StmtKind::For {
        index_pattern,
        pattern,
        ..
    } = &function.body.statements[0].kind
    else {
        panic!("expected for statement");
    };
    assert_eq!(
        index_pattern.as_ref(),
        Some(&Pattern::Binding("index".to_owned()))
    );
    let Pattern::RecordVariant { path, fields } = pattern else {
        panic!("expected record variant pattern");
    };
    assert_eq!(path, &["Reward", "Grant"]);
    assert_eq!(fields.len(), 1);
}

#[test]
fn parses_statement_attributes() {
    let parsed = parse_source(
        source_id(),
        r#"
fn main(rewards) {
    #[trace("reward")]
    let total = 0;
    #[audit]
    for reward in rewards {
        total += reward;
    }
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };

    let let_stmt = &function.body.statements[0];
    assert_eq!(let_stmt.attrs.len(), 1);
    assert_eq!(let_stmt.attrs[0].path, ["trace"]);
    assert_eq!(let_stmt.attrs[0].value.as_deref(), Some("reward"));
    assert!(matches!(let_stmt.kind, StmtKind::Let { .. }));

    let for_stmt = &function.body.statements[1];
    assert_eq!(for_stmt.attrs.len(), 1);
    assert_eq!(for_stmt.attrs[0].path, ["audit"]);
    assert_eq!(for_stmt.attrs[0].value, None);
    assert!(matches!(for_stmt.kind, StmtKind::For { .. }));
}

#[test]
fn parses_match_lambda_record_and_map_expressions() {
    let parsed = parse_source(
        source_id(),
        r#"
fn update(player) {
    let values = {"level": player.level, count: 1};
    let reward = KillReward { item_id: "gold", count };
    let mapped = values.map(|entry| entry.value + 1);
    match player.quest_progress {
        QuestProgress::Active { quest_id, count } => {
            player.quest_progress = QuestProgress::Active { quest_id, count: count + 1 };
        },
        _ => reward,
    }
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };
    assert_eq!(function.body.statements.len(), 4);

    let StmtKind::Let {
        value: Some(map), ..
    } = &function.body.statements[0].kind
    else {
        panic!("expected map let");
    };
    assert!(matches!(map.kind, ExprKind::Map(_)));

    let StmtKind::Let {
        value: Some(record),
        ..
    } = &function.body.statements[1].kind
    else {
        panic!("expected record let");
    };
    assert!(matches!(record.kind, ExprKind::Record { .. }));

    let StmtKind::Expr(Expr {
        kind: ExprKind::Match(match_expr),
        ..
    }) = &function.body.statements[3].kind
    else {
        panic!("expected match expression statement");
    };
    assert_eq!(match_expr.arms.len(), 2);
    assert!(matches!(match_expr.arms[1].pattern, Pattern::Wildcard));
}

#[test]
fn parses_zero_arg_lambda_expression() {
    let parsed = parse_source(
        source_id(),
        r#"
fn main() {
    let predicate = || true;
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };
    let StmtKind::Let {
        value: Some(lambda),
        ..
    } = &function.body.statements[0].kind
    else {
        panic!("expected lambda let");
    };
    let ExprKind::Lambda { params, .. } = &lambda.kind else {
        panic!("expected lambda");
    };
    assert!(params.is_empty());
}

#[test]
fn parser_recovers_after_bad_item() {
    let parsed = parse_source(source_id(), "bogus @@@\nfn next() {}");

    assert!(!parsed.diagnostics.is_empty());
    assert_eq!(parsed.items.len(), 1);
    assert!(matches!(parsed.items[0].kind, ItemKind::Function(_)));
}

#[test]
fn parses_literal_return() {
    let parsed = parse_source(source_id(), "fn answer() { return 42; }");

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };
    let StmtKind::Return(Some(value)) = &function.body.statements[0].kind else {
        panic!("expected return value");
    };
    assert_eq!(value.kind, ExprKind::Literal(Literal::integer("42")));
}

#[test]
fn parses_integer_literal_radix_metadata() {
    let parsed = parse_source(
        source_id(),
        "fn numbers() { let hex = 0x2a; let binary = 0b1010; }",
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };

    let StmtKind::Let {
        value: Some(hex), ..
    } = &function.body.statements[0].kind
    else {
        panic!("expected hex binding");
    };
    assert!(matches!(
        &hex.kind,
        ExprKind::Literal(Literal::Integer(value))
            if value.source_text() == "0x2a" && value.radix == IntRadix::Hex
    ));

    let StmtKind::Let {
        value: Some(binary),
        ..
    } = &function.body.statements[1].kind
    else {
        panic!("expected binary binding");
    };
    assert!(matches!(
        &binary.kind,
        ExprKind::Literal(Literal::Integer(value))
            if value.source_text() == "0b1010" && value.radix == IntRadix::Binary
    ));
}

#[test]
fn parses_numeric_literal_suffix_metadata() {
    let parsed = parse_source(
        source_id(),
        "fn numbers() { let int = 12i8; let float = 12.0f32; }",
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };

    let StmtKind::Let {
        value: Some(int), ..
    } = &function.body.statements[0].kind
    else {
        panic!("expected int binding");
    };
    assert!(matches!(
        &int.kind,
        ExprKind::Literal(Literal::Integer(value))
            if value.source_text() == "12" && value.suffix == Some(IntegerSuffix::I8)
    ));

    let StmtKind::Let {
        value: Some(float), ..
    } = &function.body.statements[1].kind
    else {
        panic!("expected float binding");
    };
    assert!(matches!(
        &float.kind,
        ExprKind::Literal(Literal::Float(value))
            if value.source_text() == "12.0" && value.suffix == Some(FloatSuffix::F32)
    ));
}

#[test]
fn parses_range_expressions() {
    let parsed = parse_source(
        source_id(),
        r#"
fn main() {
    let exclusive = 1..4;
    let inclusive = 1..=4;
    return inclusive;
}
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let ItemKind::Function(function) = &parsed.items[0].kind else {
        panic!("expected function item");
    };
    let StmtKind::Let {
        value: Some(exclusive),
        ..
    } = &function.body.statements[0].kind
    else {
        panic!("expected exclusive range let");
    };
    assert!(matches!(
        exclusive.kind,
        ExprKind::Binary {
            op: BinaryOp::Range,
            ..
        }
    ));
    let StmtKind::Let {
        value: Some(inclusive),
        ..
    } = &function.body.statements[1].kind
    else {
        panic!("expected inclusive range let");
    };
    assert!(matches!(
        inclusive.kind,
        ExprKind::Binary {
            op: BinaryOp::RangeInclusive,
            ..
        }
    ));
}
