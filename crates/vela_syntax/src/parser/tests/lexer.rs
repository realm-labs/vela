use super::*;

#[test]
fn lexes_keywords_identifiers_and_operators_with_spans() {
    let lexed = lex(
        source_id(),
        "pub fn level_up(player) { player.level += 1; 1..10; 1..=10 }",
    );

    assert!(lexed.diagnostics.is_empty());
    assert_eq!(lexed.tokens[0].kind, TokenKind::Keyword(Keyword::Pub));
    assert_eq!(lexed.tokens[0].span, Span::new(source_id(), 0, 3));
    assert_eq!(lexed.tokens[2].kind, TokenKind::Ident("level_up".into()));
    assert!(
        lexed
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Symbol(Symbol::PlusEqual))
    );
    assert!(
        lexed
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Symbol(Symbol::DotDot))
    );
    assert!(
        lexed
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Symbol(Symbol::DotDotEqual))
    );
}

#[test]
fn lexes_radix_ints_and_exponent_floats() {
    let lexed = lex(source_id(), "0x2a 0b1010 1_000 3.5e+2 4.25E-1");

    assert!(lexed.diagnostics.is_empty());
    assert_eq!(lexed.tokens[0].kind, int_token("0x2a", IntRadix::Hex, None));
    assert_eq!(
        lexed.tokens[1].kind,
        int_token("0b1010", IntRadix::Binary, None)
    );
    assert_eq!(
        lexed.tokens[2].kind,
        int_token("1_000", IntRadix::Decimal, None)
    );
    assert_eq!(lexed.tokens[3].kind, float_token("3.5e+2", None));
    assert_eq!(lexed.tokens[4].kind, float_token("4.25E-1", None));
}

#[test]
fn lexes_numeric_suffixes() {
    let lexed = lex(
        source_id(),
        "12i8 12i16 12i32 12i64 12u8 12u16 12u32 12u64 12.0f32 12.0f64 0xffu8 0b1010u16",
    );

    assert!(lexed.diagnostics.is_empty());
    assert_eq!(
        lexed
            .tokens
            .iter()
            .map(|token| token.kind.clone())
            .take(12)
            .collect::<Vec<_>>(),
        vec![
            int_token("12", IntRadix::Decimal, Some(IntegerSuffix::I8)),
            int_token("12", IntRadix::Decimal, Some(IntegerSuffix::I16)),
            int_token("12", IntRadix::Decimal, Some(IntegerSuffix::I32)),
            int_token("12", IntRadix::Decimal, Some(IntegerSuffix::I64)),
            int_token("12", IntRadix::Decimal, Some(IntegerSuffix::U8)),
            int_token("12", IntRadix::Decimal, Some(IntegerSuffix::U16)),
            int_token("12", IntRadix::Decimal, Some(IntegerSuffix::U32)),
            int_token("12", IntRadix::Decimal, Some(IntegerSuffix::U64)),
            float_token("12.0", Some(FloatSuffix::F32)),
            float_token("12.0", Some(FloatSuffix::F64)),
            int_token("0xff", IntRadix::Hex, Some(IntegerSuffix::U8)),
            int_token("0b1010", IntRadix::Binary, Some(IntegerSuffix::U16)),
        ]
    );
}

#[test]
fn diagnoses_invalid_numeric_suffixes_without_trailing_identifier_tokens() {
    let lexed = lex(source_id(), "12i128 12usize 12abc 12.0i32");

    assert_eq!(
        lexed
            .tokens
            .iter()
            .map(|token| token.kind.clone())
            .collect::<Vec<_>>(),
        vec![
            int_token("12", IntRadix::Decimal, None),
            int_token("12", IntRadix::Decimal, None),
            int_token("12", IntRadix::Decimal, None),
            float_token("12.0", None),
            TokenKind::Eof,
        ]
    );
    assert_eq!(
        lexed
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_deref())
            .collect::<Vec<_>>(),
        vec![
            Some("E_LEX_NUMERIC_SUFFIX"),
            Some("E_LEX_NUMERIC_SUFFIX"),
            Some("E_LEX_NUMERIC_SUFFIX"),
            Some("E_LEX_NUMERIC_SUFFIX"),
        ]
    );
}

#[test]
fn diagnoses_radix_ints_without_digits() {
    let lexed = lex(source_id(), "0x 0x_ 0b 0b_");

    assert_eq!(lexed.tokens[0].kind, int_token("0x", IntRadix::Hex, None));
    assert_eq!(lexed.tokens[1].kind, int_token("0x_", IntRadix::Hex, None));
    assert_eq!(
        lexed.tokens[2].kind,
        int_token("0b", IntRadix::Binary, None)
    );
    assert_eq!(
        lexed.tokens[3].kind,
        int_token("0b_", IntRadix::Binary, None)
    );
    assert_eq!(lexed.diagnostics.len(), 4);
    assert!(
        lexed
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code.as_deref() == Some("E_LEX_INT"))
    );
    assert_eq!(
        lexed
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.span)
            .collect::<Vec<_>>(),
        vec![
            Some(Span::new(source_id(), 0, 2)),
            Some(Span::new(source_id(), 3, 6)),
            Some(Span::new(source_id(), 7, 9)),
            Some(Span::new(source_id(), 10, 13)),
        ]
    );
}

#[test]
fn diagnoses_uppercase_radix_prefixes() {
    let lexed = lex(source_id(), "0X2a 0B1010");

    assert_eq!(lexed.tokens[0].kind, int_token("0X2a", IntRadix::Hex, None));
    assert_eq!(
        lexed.tokens[1].kind,
        int_token("0B1010", IntRadix::Binary, None)
    );
    assert_eq!(
        lexed
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("E_LEX_INT"), Some("E_LEX_INT")]
    );
    assert_eq!(
        lexed
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.span)
            .collect::<Vec<_>>(),
        vec![
            Some(Span::new(source_id(), 0, 4)),
            Some(Span::new(source_id(), 5, 11)),
        ]
    );
}

#[test]
fn lexes_leading_shebang_as_layout() {
    let lexed = lex(source_id(), "#!/usr/bin/env vela\nfn main() { return 1; }");

    assert!(lexed.diagnostics.is_empty());
    assert_eq!(lexed.tokens[0].kind, TokenKind::Keyword(Keyword::Fn));
    assert_eq!(
        lexed.tokens[0].span,
        Span::new(source_id(), "#!/usr/bin/env vela\n".len() as u32, 22)
    );
}

#[test]
fn lexes_unicode_string_escapes() {
    let lexed = lex(source_id(), r#""\u{41}\u{7a}""#);

    assert!(lexed.diagnostics.is_empty());
    assert_eq!(lexed.tokens[0].kind, TokenKind::String("Az".into()));
}

#[test]
fn diagnoses_invalid_string_escapes() {
    let lexed = lex(source_id(), r#""quest\qtag""#);

    assert_eq!(lexed.tokens[0].kind, TokenKind::String("questqtag".into()));
    assert_eq!(lexed.diagnostics.len(), 1);
    assert_eq!(
        lexed.diagnostics[0].code.as_deref(),
        Some("E_LEX_STRING_ESCAPE")
    );
    assert_eq!(
        lexed.diagnostics[0].span,
        Some(Span::new(source_id(), 6, 8))
    );
}

#[test]
fn lexes_byte_strings_with_allowed_escapes() {
    let lexed = lex(source_id(), r#"b"az\n\r\t\0\"\\\x00\xff" b"plain""#);

    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    assert_eq!(
        lexed.tokens[0].kind,
        TokenKind::Bytes(vec![
            b'a', b'z', b'\n', b'\r', b'\t', b'\0', b'"', b'\\', 0, 255
        ])
    );
    assert_eq!(
        lexed.tokens[0].span,
        Span::new(source_id(), 0, r#"b"az\n\r\t\0\"\\\x00\xff""#.len() as u32)
    );
    assert_eq!(lexed.tokens[1].kind, TokenKind::Bytes(b"plain".to_vec()));
}

#[test]
fn diagnoses_invalid_byte_string_escapes() {
    let lexed = lex(source_id(), r#"b"\q" b"\xg0" b"\x0" "#);

    assert_eq!(lexed.tokens[0].kind, TokenKind::Bytes(vec![b'q']));
    assert_eq!(lexed.tokens[1].kind, TokenKind::Bytes(vec![b'0']));
    assert_eq!(lexed.tokens[2].kind, TokenKind::Bytes(Vec::new()));
    assert_eq!(
        lexed
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_deref())
            .collect::<Vec<_>>(),
        vec![
            Some("E_LEX_BYTE_ESCAPE"),
            Some("E_LEX_BYTE_ESCAPE"),
            Some("E_LEX_BYTE_ESCAPE"),
        ]
    );
}

#[test]
fn diagnoses_unicode_byte_strings() {
    let lexed = lex(source_id(), r#"b"\u{41}" b"é""#);

    assert_eq!(lexed.tokens[0].kind, TokenKind::Bytes(Vec::new()));
    assert_eq!(lexed.tokens[1].kind, TokenKind::Bytes(Vec::new()));
    assert_eq!(
        lexed
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("E_LEX_BYTE_ESCAPE"), Some("E_LEX_BYTE_CHAR")]
    );
}
