use super::*;
use vela_common::SourceId;
use vela_hir::body::{HirExprKind, HirPatternKind};
use vela_hir::module_graph::{ModulePath, ModuleSource};

use crate::facts::AnalysisFacts;
use crate::type_fact::TypeFact;

fn integer(text: &str, radix: HirIntRadix, suffix: Option<HirIntegerSuffix>) -> HirIntegerLiteral {
    HirIntegerLiteral {
        text: text.to_owned(),
        radix,
        suffix,
    }
}

fn float(text: &str, suffix: Option<HirFloatSuffix>) -> HirFloatLiteral {
    HirFloatLiteral {
        text: text.to_owned(),
        suffix,
    }
}

fn scalar(result: LiteralResult) -> ScalarValue {
    result
        .expect("literal should resolve")
        .scalar()
        .expect("literal should be a scalar")
}

#[test]
fn resolves_every_integer_suffix_and_stable_spelling() {
    let cases = [
        (HirIntegerSuffix::I8, ScalarValue::I8(12)),
        (HirIntegerSuffix::I16, ScalarValue::I16(12)),
        (HirIntegerSuffix::I32, ScalarValue::I32(12)),
        (HirIntegerSuffix::I64, ScalarValue::I64(12)),
        (HirIntegerSuffix::U8, ScalarValue::U8(12)),
        (HirIntegerSuffix::U16, ScalarValue::U16(12)),
        (HirIntegerSuffix::U32, ScalarValue::U32(12)),
        (HirIntegerSuffix::U64, ScalarValue::U64(12)),
    ];
    for (suffix, expected) in cases {
        let literal = integer("12", HirIntRadix::Decimal, Some(suffix));
        assert_eq!(
            scalar(resolve_integer_literal(
                &literal,
                LiteralPrimitiveContext::Default,
                LiteralSign::Positive,
            )),
            expected
        );
        assert!(integer_literal_spelling(&literal).ends_with(expected.type_name()));
    }

    let binary = integer("0b1010", HirIntRadix::Binary, Some(HirIntegerSuffix::U8));
    let hex = integer("0xff", HirIntRadix::Hex, Some(HirIntegerSuffix::U16));
    assert_eq!(integer_literal_spelling(&binary), "0b1010u8");
    assert_eq!(integer_literal_spelling(&hex), "0xffu16");
    assert_eq!(
        scalar(resolve_integer_literal(
            &binary,
            LiteralPrimitiveContext::Default,
            LiteralSign::Positive
        )),
        ScalarValue::U8(10)
    );
    assert_eq!(
        scalar(resolve_integer_literal(
            &hex,
            LiteralPrimitiveContext::Default,
            LiteralSign::Positive
        )),
        ScalarValue::U16(255)
    );
}

#[test]
fn resolves_defaults_and_contextual_primitives() {
    let integer = integer("255", HirIntRadix::Decimal, None);
    let float = float("12.5", None);
    assert_eq!(
        scalar(resolve_integer_literal(
            &integer,
            LiteralPrimitiveContext::Default,
            LiteralSign::Positive
        )),
        ScalarValue::I64(255)
    );
    assert_eq!(
        scalar(resolve_integer_literal(
            &integer,
            LiteralPrimitiveContext::Expected(PrimitiveTag::U8),
            LiteralSign::Positive
        )),
        ScalarValue::U8(255)
    );
    assert_eq!(
        scalar(resolve_float_literal(
            &float,
            LiteralPrimitiveContext::Default,
            LiteralSign::Positive
        )),
        ScalarValue::F64(12.5)
    );
    assert_eq!(
        scalar(resolve_float_literal(
            &float,
            LiteralPrimitiveContext::Expected(PrimitiveTag::F32),
            LiteralSign::Positive
        )),
        ScalarValue::F32(12.5)
    );
}

#[test]
fn accepts_signed_minima_and_classifies_both_overflow_directions() {
    let minima = [
        ("128", HirIntegerSuffix::I8, ScalarValue::I8(i8::MIN)),
        ("32768", HirIntegerSuffix::I16, ScalarValue::I16(i16::MIN)),
        (
            "2147483648",
            HirIntegerSuffix::I32,
            ScalarValue::I32(i32::MIN),
        ),
        (
            "9223372036854775808",
            HirIntegerSuffix::I64,
            ScalarValue::I64(i64::MIN),
        ),
    ];
    for (text, suffix, expected) in minima {
        let literal = integer(text, HirIntRadix::Decimal, Some(suffix));
        assert_eq!(
            scalar(resolve_integer_literal(
                &literal,
                LiteralPrimitiveContext::Default,
                LiteralSign::Negated
            )),
            expected
        );
    }

    for sign in [LiteralSign::Positive, LiteralSign::Negated] {
        let literal = integer("129", HirIntRadix::Decimal, Some(HirIntegerSuffix::I8));
        let error = resolve_integer_literal(&literal, LiteralPrimitiveContext::Default, sign)
            .expect_err("129i8 cannot be positive or negated i8");
        assert_eq!(error.class(), LiteralErrorClass::OutOfRange);
        assert_eq!(error.detail(), "integer literal out of range");
    }
}

#[test]
fn validates_float_widths_and_overflow() {
    for (suffix, expected) in [
        (HirFloatSuffix::F32, ScalarValue::F32(1.25)),
        (HirFloatSuffix::F64, ScalarValue::F64(1.25)),
    ] {
        let literal = float("1.25", Some(suffix));
        assert_eq!(
            scalar(resolve_float_literal(
                &literal,
                LiteralPrimitiveContext::Default,
                LiteralSign::Positive
            )),
            expected
        );
        assert_eq!(
            float_literal_spelling(&literal),
            format!("1.25{}", expected.type_name())
        );
    }

    for literal in [
        float("3.5e38", Some(HirFloatSuffix::F32)),
        float("1.8e308", Some(HirFloatSuffix::F64)),
    ] {
        let error = resolve_float_literal(
            &literal,
            LiteralPrimitiveContext::Default,
            LiteralSign::Positive,
        )
        .expect_err("non-finite float should be rejected");
        assert_eq!(error.class(), LiteralErrorClass::OutOfRange);
        assert_eq!(error.detail(), "float literal out of range");
    }
}

#[test]
fn classifies_invalid_digits_and_incompatible_contracts() {
    let invalid = integer("0b102", HirIntRadix::Binary, None);
    let error = resolve_integer_literal(
        &invalid,
        LiteralPrimitiveContext::Default,
        LiteralSign::Positive,
    )
    .expect_err("invalid binary digit should fail");
    assert_eq!(error.class(), LiteralErrorClass::InvalidDigits);

    let suffixed = integer("1", HirIntRadix::Decimal, Some(HirIntegerSuffix::I8));
    let error = resolve_integer_literal(
        &suffixed,
        LiteralPrimitiveContext::Expected(PrimitiveTag::U8),
        LiteralSign::Positive,
    )
    .expect_err("a suffix cannot be contextually changed");
    assert_eq!(error.class(), LiteralErrorClass::IncompatiblePrimitive);
    assert_eq!(error.spelling(), "1i8");
    assert!(
        error
            .to_compiler_diagnostic(Span::new(SourceId::new(1), 0, 3))
            .is_none()
    );

    let float = float("1.0", None);
    assert_eq!(
        resolve_float_literal(
            &float,
            LiteralPrimitiveContext::Expected(PrimitiveTag::I64),
            LiteralSign::Positive
        )
        .expect_err("float cannot satisfy integer context")
        .class(),
        LiteralErrorClass::IncompatiblePrimitive
    );
}

#[test]
fn retains_validated_deferred_dynamic_literals() {
    let dynamic_integer = integer("18_446_744_073_709_551_615", HirIntRadix::Decimal, None);
    let resolved = resolve_integer_literal(
        &dynamic_integer,
        LiteralPrimitiveContext::DeferredDynamic,
        LiteralSign::Positive,
    )
    .expect("u64 maximum may be selected by a dynamic operand");
    let deferred = resolved.deferred().expect("dynamic literal should defer");
    assert_eq!(deferred.kind(), NumericLiteralKind::Integer);
    assert_eq!(deferred.text(), "18_446_744_073_709_551_615");

    let too_large = integer("18446744073709551616", HirIntRadix::Decimal, None);
    assert_eq!(
        resolve_integer_literal(
            &too_large,
            LiteralPrimitiveContext::DeferredDynamic,
            LiteralSign::Positive
        )
        .expect_err("no language primitive can hold the value")
        .class(),
        LiteralErrorClass::OutOfRange
    );

    let float = float("1.5", None);
    assert_eq!(
        resolve_float_literal(
            &float,
            LiteralPrimitiveContext::DeferredDynamic,
            LiteralSign::Positive
        )
        .expect("finite dynamic float")
        .deferred()
        .expect("dynamic float should defer")
        .text(),
        "1.5"
    );
}

#[test]
fn facts_key_signed_minimum_by_literal_and_unary_but_not_unsigned_negation() {
    const SOURCE: &str = "fn main() { let a = -128i8; let b = -1u8; return a; }";
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(1),
        ModulePath::from_qualified("main"),
        SOURCE,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let facts = LiteralFacts::from_module_graph(&graph);

    let body = graph.bodies().next().expect("main body");
    let mut signed = None;
    let mut unsigned = None;
    for (expression, record) in &body.expressions {
        if let HirExprKind::Unary {
            op: Some(HirUnaryOp::Negate),
            operand: Some(operand),
        } = record.kind
        {
            match body.expression(operand).map(|record| &record.kind) {
                Some(HirExprKind::Literal(HirLiteral::Integer(value)))
                    if value.suffix == Some(HirIntegerSuffix::I8) =>
                {
                    signed = Some((*expression, operand));
                }
                Some(HirExprKind::Literal(HirLiteral::Integer(value)))
                    if value.suffix == Some(HirIntegerSuffix::U8) =>
                {
                    unsigned = Some((*expression, operand));
                }
                _ => {}
            }
        }
    }
    let (signed_unary, signed_literal) = signed.expect("signed negation");
    assert_eq!(facts.get(signed_literal), None);
    assert_eq!(
        facts
            .get(signed_unary)
            .and_then(|result| result.as_ref().ok())
            .and_then(ResolvedLiteralFact::scalar),
        Some(ScalarValue::I8(i8::MIN))
    );

    let (unsigned_unary, unsigned_literal) = unsigned.expect("unsigned negation");
    assert_eq!(
        facts
            .get(unsigned_literal)
            .and_then(|result| result.as_ref().ok())
            .and_then(ResolvedLiteralFact::scalar),
        Some(ScalarValue::U8(1))
    );
    assert_eq!(facts.get(unsigned_unary), None);
}

#[test]
fn negated_overflow_is_keyed_by_unary_with_operand_diagnostic_origin() {
    const SOURCE: &str = "const BAD = -129i8; fn main() { return BAD; }";
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(9),
        ModulePath::from_qualified("main"),
        SOURCE,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let facts = LiteralFacts::from_module_graph(&graph);
    let body = graph
        .bodies()
        .find(|body| {
            matches!(
                body.owner,
                vela_hir::body::HirBodyOwner::ConstInitializer(_)
            )
        })
        .expect("const initializer body");
    let (unary, literal) = body
        .expressions
        .iter()
        .find_map(|(expression, record)| match record.kind {
            HirExprKind::Unary {
                op: Some(HirUnaryOp::Negate),
                operand: Some(operand),
            } => Some((*expression, operand)),
            _ => None,
        })
        .expect("negated literal");
    assert_eq!(facts.get(literal), None);
    assert_eq!(
        facts
            .get(unary)
            .and_then(|result| result.as_ref().err())
            .map(LiteralError::class),
        Some(LiteralErrorClass::OutOfRange)
    );
    let diagnostics = facts.compiler_diagnostics(&graph);
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("expected one negated overflow diagnostic");
    };
    let span = diagnostic.span.expect("diagnostic span");
    assert_eq!(&SOURCE[span.start as usize..span.end as usize], "129i8");
}

#[test]
fn contextual_facts_and_diagnostics_preserve_hir_ids_and_spans() {
    const SOURCE: &str = "fn main() { let value: u8 = 255; return 128i8; }";
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(7),
        ModulePath::from_qualified("main"),
        SOURCE,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let body = graph.bodies().next().expect("main body");
    let (unsuffixed, invalid) = body
        .expressions
        .iter()
        .filter_map(|(id, record)| match &record.kind {
            HirExprKind::Literal(HirLiteral::Integer(value)) if value.suffix.is_none() => {
                Some((*id, false))
            }
            HirExprKind::Literal(HirLiteral::Integer(value))
                if value.suffix == Some(HirIntegerSuffix::I8) =>
            {
                Some((*id, true))
            }
            _ => None,
        })
        .fold((None, None), |(plain, bad), (id, is_bad)| {
            if is_bad {
                (plain, Some(id))
            } else {
                (Some(id), bad)
            }
        });
    let unsuffixed = unsuffixed.expect("unsuffixed literal");
    let invalid = invalid.expect("invalid literal");
    let mut contexts = BTreeMap::new();
    contexts.insert(
        unsuffixed,
        LiteralPrimitiveContext::Expected(PrimitiveTag::U8),
    );
    let facts = LiteralFacts::from_module_graph_with_contexts(&graph, &contexts);
    assert_eq!(
        facts
            .get(unsuffixed)
            .and_then(|result| result.as_ref().ok())
            .and_then(ResolvedLiteralFact::scalar),
        Some(ScalarValue::U8(255))
    );
    assert_eq!(
        facts
            .get(invalid)
            .and_then(|result| result.as_ref().err())
            .map(LiteralError::class),
        Some(LiteralErrorClass::OutOfRange)
    );

    let diagnostics = facts.compiler_diagnostics(&graph);
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("expected one literal diagnostic");
    };
    assert_eq!(
        diagnostic.code.as_deref(),
        Some("compiler::invalid_int_literal")
    );
    assert_eq!(
        diagnostic.message,
        "invalid integer literal `128i8`: integer literal out of range"
    );
    let span = diagnostic.span.expect("diagnostic span");
    assert_eq!(&SOURCE[span.start as usize..span.end as usize], "128i8");
}

#[test]
fn numeric_literal_use_classifies_frozen_dynamic_operator_shapes() {
    const SOURCE: &str = r#"
fn main(value) {
    let positive = value + 1;
    let parenthesized = value + (2);
    let negated = value + -3;
    let equality = value == 4;
}
"#;
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        SourceId::new(10),
        ModulePath::from_qualified("main"),
        SOURCE,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let body = graph.bodies().next().expect("main body");

    let binary_rhs = |source_text: &str| {
        body.expressions
            .values()
            .find_map(|expression| {
                let span = expression.origin.span;
                let text = &SOURCE[span.start as usize..span.end as usize];
                if text != source_text {
                    return None;
                }
                match expression.kind {
                    HirExprKind::Binary {
                        op: Some(op),
                        rhs: Some(rhs),
                        ..
                    } => Some((op, rhs)),
                    _ => None,
                }
            })
            .unwrap_or_else(|| panic!("binary expression `{source_text}`"))
    };

    let (add, positive) = binary_rhs("value + 1");
    let positive = NumericLiteralUse::classify(body, positive).expect("positive literal use");
    assert_eq!(positive.kind(), NumericLiteralKind::Integer);
    assert_eq!(positive.sign(), LiteralSign::Positive);
    assert!(!positive.is_parenthesized());
    assert!(positive.supports_direct_contract_context());
    assert!(positive.supports_deferred_operation(add));

    let (add, parenthesized) = binary_rhs("value + (2)");
    let parenthesized =
        NumericLiteralUse::classify(body, parenthesized).expect("parenthesized literal use");
    assert!(parenthesized.is_parenthesized());
    assert!(!parenthesized.supports_direct_contract_context());
    assert!(parenthesized.supports_deferred_operation(add));

    let (add, negated) = binary_rhs("value + -3");
    let negated = NumericLiteralUse::classify(body, negated).expect("negated literal use");
    assert_eq!(negated.sign(), LiteralSign::Negated);
    assert!(!negated.supports_deferred_operation(add));
    assert_ne!(
        negated.literal_expression(),
        negated.resolution_expression()
    );

    let (equal, equality) = binary_rhs("value == 4");
    let equality = NumericLiteralUse::classify(body, equality).expect("equality literal use");
    assert_eq!(equality.sign(), LiteralSign::Positive);
    assert!(!equality.supports_deferred_operation(equal));
    assert!(!supports_deferred_numeric_literal(equal));
}

#[test]
fn pattern_literal_facts_validate_ranges_and_preserve_pattern_spans() {
    const SOURCE: &str = r#"
fn main(value) {
    return match value {
        128i8 => 1,
        2u8 => 2,
        _ => 0,
    };
}
"#;
    let source = SourceId::new(11);
    let mut graph = ModuleGraph::new();
    graph.add_source(ModuleSource::new(
        source,
        ModulePath::from_qualified("main"),
        SOURCE,
    ));
    graph.resolve_imports();
    assert_eq!(graph.diagnostics(), &[]);
    let body = graph.bodies().next().expect("main body");
    let mut invalid = None;
    let mut valid = None;
    for pattern in body.patterns.values() {
        let HirPatternKind::Literal(Some(HirLiteral::Integer(literal))) = &pattern.kind else {
            continue;
        };
        match literal.text.as_str() {
            "128" => invalid = Some(pattern.id),
            "2" => valid = Some(pattern.id),
            _ => {}
        }
    }
    let invalid = invalid.expect("invalid i8 pattern");
    let valid = valid.expect("valid u8 pattern");
    let facts = LiteralFacts::from_module_graph(&graph);
    assert!(matches!(
        facts.pattern(invalid),
        Some(Err(error)) if error.class() == LiteralErrorClass::OutOfRange
    ));
    assert_eq!(
        facts
            .pattern(valid)
            .and_then(|result| result.as_ref().ok())
            .and_then(ResolvedLiteralFact::scalar),
        Some(ScalarValue::U8(2))
    );

    let analysis = AnalysisFacts::from_module_graph(&graph);
    assert_eq!(analysis.pattern(invalid), Some(&TypeFact::Unknown));
    assert_eq!(analysis.pattern(valid), Some(&TypeFact::U8));
    assert!(analysis.pattern_literal(invalid).is_some());

    let diagnostics = facts.compiler_diagnostics(&graph);
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("expected one invalid pattern literal diagnostic");
    };
    assert_eq!(
        diagnostic.code.as_deref(),
        Some("compiler::invalid_int_literal")
    );
    assert_eq!(
        diagnostic.message,
        "invalid integer literal `128i8`: integer literal out of range"
    );
    let start = SOURCE.find("128i8").expect("invalid pattern start") as u32;
    let expected_span = Span::new(source, start, start + "128i8".len() as u32);
    assert_eq!(diagnostic.span, Some(expected_span));
    assert_eq!(graph.pattern_span(invalid), Some(expected_span));
}
