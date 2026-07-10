use super::*;
use vela_common::{Diagnostic, Severity};

#[derive(Clone, Copy)]
struct ExpectedLabel<'a> {
    span_text: &'a str,
    message: &'a str,
}

#[derive(Clone, Copy)]
struct ExpectedLabelOccurrence<'a> {
    span_text: &'a str,
    occurrence: usize,
    message: &'a str,
}

#[derive(Clone, Copy)]
struct ExpectedSpanOccurrence<'a> {
    text: &'a str,
    occurrence: usize,
}

fn only_syntax_diagnostic(error: CompileError) -> Diagnostic {
    let CompileErrorKind::SyntaxDiagnostics(mut diagnostics) = error.kind else {
        panic!("expected syntax diagnostics, got {error:?}");
    };
    assert_eq!(
        diagnostics.len(),
        1,
        "unexpected diagnostics: {diagnostics:?}"
    );
    diagnostics.remove(0)
}

fn only_semantic_diagnostic(error: CompileError) -> Diagnostic {
    let CompileErrorKind::SemanticDiagnostics(mut diagnostics) = error.kind else {
        panic!("expected semantic diagnostics, got {error:?}");
    };
    assert_eq!(
        diagnostics.len(),
        1,
        "unexpected diagnostics: {diagnostics:?}"
    );
    diagnostics.remove(0)
}

fn source_span(source_id: SourceId, source: &str, text: &str) -> Span {
    let mut matches = source.match_indices(text);
    let (start, _) = matches
        .next()
        .unwrap_or_else(|| panic!("`{text}` is absent from diagnostic fixture source"));
    assert!(
        matches.next().is_none(),
        "`{text}` must be unique in diagnostic fixture source"
    );
    Span::new(
        source_id,
        u32::try_from(start).expect("fixture offset should fit in u32"),
        u32::try_from(start + text.len()).expect("fixture offset should fit in u32"),
    )
}

fn source_span_occurrence(
    source_id: SourceId,
    source: &str,
    text: &str,
    occurrence: usize,
) -> Span {
    let (start, _) = source
        .match_indices(text)
        .nth(occurrence)
        .unwrap_or_else(|| panic!("occurrence {occurrence} of `{text}` is absent from source"));
    Span::new(
        source_id,
        u32::try_from(start).expect("fixture offset should fit in u32"),
        u32::try_from(start + text.len()).expect("fixture offset should fit in u32"),
    )
}

fn assert_diagnostic_contract(
    diagnostic: &Diagnostic,
    source_id: SourceId,
    source: &str,
    code: &str,
    message: &str,
    primary_span_text: &str,
    labels: &[ExpectedLabel<'_>],
) {
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.code.as_deref(), Some(code));
    assert_eq!(diagnostic.message, message);
    assert_eq!(
        diagnostic.span,
        Some(source_span(source_id, source, primary_span_text))
    );
    let expected_labels = labels
        .iter()
        .map(|label| {
            (
                source_span(source_id, source, label.span_text),
                label.message,
            )
        })
        .collect::<Vec<_>>();
    let actual_labels = diagnostic
        .labels
        .iter()
        .map(|label| (label.span, label.message.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(actual_labels, expected_labels);

    // The current compiler boundary conveys suggestions through labels. Pin the
    // structured fields too so a later ownership move must update this fixture.
    assert!(diagnostic.candidates.is_empty());
    assert!(diagnostic.repairs.is_empty());
}

fn assert_diagnostic_contract_with_occurrences(
    diagnostic: &Diagnostic,
    source_id: SourceId,
    source: &str,
    code: &str,
    message: &str,
    primary: ExpectedSpanOccurrence<'_>,
    labels: &[ExpectedLabelOccurrence<'_>],
) {
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.code.as_deref(), Some(code));
    assert_eq!(diagnostic.message, message);
    assert_eq!(
        diagnostic.span,
        Some(source_span_occurrence(
            source_id,
            source,
            primary.text,
            primary.occurrence,
        ))
    );
    let expected_labels = labels
        .iter()
        .map(|label| {
            (
                source_span_occurrence(source_id, source, label.span_text, label.occurrence),
                label.message,
            )
        })
        .collect::<Vec<_>>();
    let actual_labels = diagnostic
        .labels
        .iter()
        .map(|label| (label.span, label.message.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(actual_labels, expected_labels);
    assert!(diagnostic.candidates.is_empty());
    assert!(diagnostic.repairs.is_empty());
}

#[test]
fn internal_input_diagnostic_contracts_pin_projection_and_source_ownership() {
    const SOURCE_ID: SourceId = SourceId::new(721);
    const SOURCE: &str = "fn main() { return 1; }";
    let span = source_span(SOURCE_ID, SOURCE, "return 1");
    let origin = vela_mir::MirSourceOrigin::body(vela_hir::ids::HirBodyId::new(721), span);
    let error = CompileError {
        kind: CompileErrorKind::MirInput(Box::new(vela_mir::MirBuildError::InconsistentInput {
            origin,
            message: "fixture target is missing".to_owned(),
        })),
        span: Some(span),
    };
    let diagnostic = error
        .to_diagnostic()
        .expect("MIR input failures should project to a diagnostic");
    assert_diagnostic_contract(
        &diagnostic,
        SOURCE_ID,
        SOURCE,
        "compiler::inconsistent_mir_input",
        "inconsistent compiler MIR input: inconsistent MIR input at 721:12..20: fixture target is missing",
        "return 1",
        &[],
    );

    let error = CompileError {
        kind: CompileErrorKind::RegistrySnapshot(
            "required definition metadata is missing".to_owned(),
        ),
        span: None,
    };
    let diagnostic = error
        .to_diagnostic()
        .expect("registry snapshot failures should project to a diagnostic");
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(
        diagnostic.code.as_deref(),
        Some("compiler::invalid_registry_snapshot")
    );
    assert_eq!(
        diagnostic.message,
        "invalid compile-target registry snapshot: required definition metadata is missing"
    );
    assert_eq!(diagnostic.span, None);
    assert!(diagnostic.labels.is_empty());
    assert!(diagnostic.candidates.is_empty());
    assert!(diagnostic.repairs.is_empty());
}

#[test]
fn syntax_diagnostic_contract_pins_removed_literal_guidance() {
    const SOURCE_ID: SourceId = SourceId::new(701);
    const SOURCE: &str = "fn main() { return null; }";
    let error = compile_program_source(SOURCE_ID, SOURCE)
        .expect_err("removed null literal should fail at the syntax gate");
    let diagnostic = only_syntax_diagnostic(error);

    assert_diagnostic_contract(
        &diagnostic,
        SOURCE_ID,
        SOURCE,
        "syntax::removed_null",
        "`null` was removed from Vela; use `()`, `Option::None`, or `Result::Err` explicitly",
        "null",
        &[ExpectedLabel {
            span_text: "null",
            message: "`null` is not an ordinary Vela value",
        }],
    );
}

#[test]
fn binding_diagnostic_contract_pins_candidate_locations() {
    const SOURCE_ID: SourceId = SourceId::new(702);
    const SOURCE: &str = "fn main(player) { return plaeyr; }";
    let error = compile_function_source(SOURCE_ID, SOURCE, "main")
        .expect_err("unresolved local should fail at the HIR binding gate");
    let diagnostic = only_semantic_diagnostic(error);

    assert_diagnostic_contract(
        &diagnostic,
        SOURCE_ID,
        SOURCE,
        "hir::unresolved_name",
        "unresolved name `plaeyr`",
        "plaeyr",
        &[
            ExpectedLabel {
                span_text: "plaeyr",
                message: "did you mean `player`?",
            },
            ExpectedLabel {
                span_text: "player",
                message: "candidate `player` is declared here",
            },
        ],
    );
}

#[test]
fn target_diagnostic_contract_pins_unresolved_compile_target() {
    const SOURCE_ID: SourceId = SourceId::new(703);
    const SOURCE: &str = "fn main() { return game::missing(1); }";
    let registry = vela_registry::DefinitionRegistry::new();
    let error = compile_program_source_with_options_and_registry(
        SOURCE_ID,
        SOURCE,
        &CompilerOptions::new(),
        registry.compile_view(),
    )
    .expect_err("unregistered native target should fail during compilation");
    let diagnostic = only_semantic_diagnostic(error);

    assert_diagnostic_contract(
        &diagnostic,
        SOURCE_ID,
        SOURCE,
        "compiler::unresolved_native_function",
        "unresolved native function `game::missing`",
        "game::missing(1)",
        &[ExpectedLabel {
            span_text: "game::missing(1)",
            message: "native function is not registered",
        }],
    );
}

#[test]
fn target_diagnostic_contract_pins_analysis_host_access_denial() {
    const SOURCE_ID: SourceId = SourceId::new(704);
    const SOURCE: &str = "fn bump(player: ReadOnlyPlayer) { player.level = 2; return 1; }";
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "ReadOnlyPlayer",
            ))
            .host_runtime_id(51),
        )
        .expect("ReadOnlyPlayer type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "ReadOnlyPlayer",
                    "level",
                ),
                player,
            )
            .type_hint(Some("i64"))
            .writable(false)
            .host_runtime_id(52),
        )
        .expect("ReadOnlyPlayer::level field should register");
    let error = compile_program_source_with_registry(SOURCE_ID, SOURCE, registry.compile_view())
        .expect_err("read-only host field assignment should fail during analysis");
    let diagnostic = only_semantic_diagnostic(error);

    assert_diagnostic_contract(
        &diagnostic,
        SOURCE_ID,
        SOURCE,
        "analysis::field_not_writable",
        "field is read-only for script writes",
        "player.level = 2",
        &[
            ExpectedLabel {
                span_text: "player.level = 2",
                message: "assignment targets a read-only field",
            },
            ExpectedLabel {
                span_text: "player.level = 2",
                message: "write through an exposed method or a writable field instead",
            },
        ],
    );
}

#[test]
fn type_contract_diagnostic_contract_pins_expected_and_actual_types() {
    const SOURCE_ID: SourceId = SourceId::new(705);
    const SOURCE: &str =
        "fn grant(amount: i64) { return amount; }\nfn main() { return grant(\"x\"); }";
    let error = compile_program_source(SOURCE_ID, SOURCE)
        .expect_err("statically incompatible script argument should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);

    assert_diagnostic_contract(
        &diagnostic,
        SOURCE_ID,
        SOURCE,
        "compiler::type_contract_mismatch",
        "type contract mismatch for parameter `amount`",
        "\"x\"",
        &[ExpectedLabel {
            span_text: "\"x\"",
            message: "expected `i64`, found `String`",
        }],
    );
}

#[test]
fn literal_diagnostic_contract_pins_contextual_conversion_failure() {
    const SOURCE_ID: SourceId = SourceId::new(706);
    const SOURCE: &str = "fn main() { return 128i8; }";
    let error = compile_function_source(SOURCE_ID, SOURCE, "main")
        .expect_err("out-of-range suffixed literal should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);

    assert_diagnostic_contract(
        &diagnostic,
        SOURCE_ID,
        SOURCE,
        "compiler::invalid_int_literal",
        "invalid integer literal `128i8`: integer literal out of range",
        "128i8",
        &[],
    );
}

#[test]
fn literal_diagnostic_contract_pins_negated_const_origin() {
    const SOURCE_ID: SourceId = SourceId::new(710);
    const SOURCE: &str = "const BAD = -129i8;\nfn main() { return BAD; }";
    let error = compile_program_source(SOURCE_ID, SOURCE)
        .expect_err("out-of-range negated const literal should fail compilation");
    let diagnostic = error
        .to_diagnostic()
        .expect("negated const conversion errors should project to a user diagnostic");

    assert_diagnostic_contract(
        &diagnostic,
        SOURCE_ID,
        SOURCE,
        "compiler::invalid_int_literal",
        "invalid integer literal `129i8`: integer literal out of range",
        "129i8",
        &[],
    );
}

#[test]
fn named_argument_diagnostic_contract_pins_parameter_candidates() {
    const SOURCE_ID: SourceId = SourceId::new(707);
    const SOURCE: &str = "fn grant(base, amount = 10) { return base + amount; }\nfn main() { return grant(amunt = 2, base = 1); }";
    let error = compile_program_source(SOURCE_ID, SOURCE)
        .expect_err("unknown named argument should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);

    assert_diagnostic_contract(
        &diagnostic,
        SOURCE_ID,
        SOURCE,
        "compiler::unknown_named_argument",
        "unknown named argument `amunt`",
        "amunt = 2",
        &[
            ExpectedLabel {
                span_text: "amunt = 2",
                message: "argument name does not match any parameter",
            },
            ExpectedLabel {
                span_text: "amunt = 2",
                message: "available parameters: amount, base",
            },
        ],
    );
}

#[test]
fn call_placement_diagnostic_contracts_pin_messages_and_source_locations() {
    const POSITIONAL_SOURCE_ID: SourceId = SourceId::new(711);
    const POSITIONAL_SOURCE: &str = "fn grant(first = 0, second = 10) { return 0; }\nfn main() { return grant(second = 2, 3); }";
    let error = compile_program_source(POSITIONAL_SOURCE_ID, POSITIONAL_SOURCE)
        .expect_err("a positional argument after a named argument should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);
    assert_diagnostic_contract(
        &diagnostic,
        POSITIONAL_SOURCE_ID,
        POSITIONAL_SOURCE,
        "compiler::positional_after_named_argument",
        "positional argument after named argument",
        "3",
        &[ExpectedLabel {
            span_text: "3",
            message: "positional arguments must appear before named arguments",
        }],
    );

    const EXTRA_SOURCE_ID: SourceId = SourceId::new(712);
    const EXTRA_SOURCE: &str = "fn grant(only) { return 0; }\nfn main() { return grant(1, 2); }";
    let error = compile_program_source(EXTRA_SOURCE_ID, EXTRA_SOURCE)
        .expect_err("an extra positional argument should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);
    assert_diagnostic_contract(
        &diagnostic,
        EXTRA_SOURCE_ID,
        EXTRA_SOURCE,
        "compiler::too_many_arguments",
        "too many arguments",
        "2",
        &[ExpectedLabel {
            span_text: "2",
            message: "call accepts 1 positional argument(s)",
        }],
    );

    const DUPLICATE_SOURCE_ID: SourceId = SourceId::new(713);
    const DUPLICATE_SOURCE: &str =
        "fn grant(base, amount = 8) { return 0; }\nfn main() { return grant(41, base = 2); }";
    let error = compile_program_source(DUPLICATE_SOURCE_ID, DUPLICATE_SOURCE)
        .expect_err("two arguments for the same parameter should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);
    assert_diagnostic_contract(
        &diagnostic,
        DUPLICATE_SOURCE_ID,
        DUPLICATE_SOURCE,
        "compiler::duplicate_argument",
        "duplicate argument for parameter `base`",
        "base = 2",
        &[
            ExpectedLabel {
                span_text: "41",
                message: "previous argument is here",
            },
            ExpectedLabel {
                span_text: "base = 2",
                message: "duplicate argument is here",
            },
        ],
    );

    const MISSING_SOURCE_ID: SourceId = SourceId::new(714);
    const MISSING_SOURCE: &str =
        "fn grant(required, optional = 10) { return 0; }\nfn main() { return grant(); }";
    let error = compile_program_source(MISSING_SOURCE_ID, MISSING_SOURCE)
        .expect_err("an omitted required argument should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);
    assert_diagnostic_contract(
        &diagnostic,
        MISSING_SOURCE_ID,
        MISSING_SOURCE,
        "compiler::missing_required_argument",
        "missing required argument `required`",
        "grant()",
        &[
            ExpectedLabel {
                span_text: "grant()",
                message: "call does not provide this required parameter",
            },
            ExpectedLabel {
                span_text: "required",
                message: "required parameter is declared here",
            },
        ],
    );
}

#[test]
fn comparison_trait_diagnostic_contracts_pin_required_traits() {
    const EQUALITY_SOURCE_ID: SourceId = SourceId::new(715);
    const EQUALITY_SOURCE: &str = "struct Reward { amount: i64 }\nfn main() { let left = Reward { amount: 1 }; let right = Reward { amount: 1 }; return left == right; }";
    let error = compile_program_source(EQUALITY_SOURCE_ID, EQUALITY_SOURCE)
        .expect_err("record equality without PartialEq should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);
    assert_diagnostic_contract(
        &diagnostic,
        EQUALITY_SOURCE_ID,
        EQUALITY_SOURCE,
        "compiler::missing_comparison_trait",
        "`Reward` does not implement `PartialEq` for `==`",
        "left == right",
        &[
            ExpectedLabel {
                span_text: "left == right",
                message: "static `==` comparison requires `PartialEq`",
            },
            ExpectedLabel {
                span_text: "left == right",
                message: "add `impl PartialEq for Reward` or make the value dynamic",
            },
        ],
    );

    const ORDERING_SOURCE_ID: SourceId = SourceId::new(716);
    const ORDERING_SOURCE: &str = "struct Score { value: i64 }\nfn main() { let low = Score { value: 1 }; let high = Score { value: 2 }; return low < high; }";
    let error = compile_program_source(ORDERING_SOURCE_ID, ORDERING_SOURCE)
        .expect_err("record ordering without PartialOrd should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);
    assert_diagnostic_contract(
        &diagnostic,
        ORDERING_SOURCE_ID,
        ORDERING_SOURCE,
        "compiler::missing_comparison_trait",
        "`Score` does not implement `PartialOrd` for `<`",
        "low < high",
        &[
            ExpectedLabel {
                span_text: "low < high",
                message: "static `<` comparison requires `PartialOrd`",
            },
            ExpectedLabel {
                span_text: "low < high",
                message: "add `impl PartialOrd for Score` or make the value dynamic",
            },
        ],
    );
}

#[test]
fn array_ord_diagnostic_contract_pins_element_requirement() {
    const SOURCE_ID: SourceId = SourceId::new(717);
    const SOURCE: &str = "struct Score { value: i64 }\nfn main() { let values = [Score { value: 2 }, Score { value: 1 }]; return values.sort(); }";
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let error = compile_program_source_with_registry(SOURCE_ID, SOURCE, registry.compile_view())
        .expect_err("sorting record values without Ord should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);

    assert_diagnostic_contract(
        &diagnostic,
        SOURCE_ID,
        SOURCE,
        "compiler::missing_ord_for_array_ordering",
        "`Array.sort` requires an `Ord` element, but `Score` does not implement `Ord`",
        "values.sort()",
        &[
            ExpectedLabel {
                span_text: "values.sort()",
                message: "static `Array.sort` requires `Ord`",
            },
            ExpectedLabel {
                span_text: "values.sort()",
                message: "add `impl Ord for Score` or use a dynamic value",
            },
        ],
    );
}

#[test]
fn constructor_diagnostic_contract_pins_available_fields() {
    const SOURCE_ID: SourceId = SourceId::new(708);
    const SOURCE: &str = "struct Reward { item_id: String, count: i64 }\nfn main() { return Reward { item_id: \"gold\", count: 2, bonus: 5 }; }";
    let error = compile_program_source(SOURCE_ID, SOURCE)
        .expect_err("unknown record constructor field should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);

    assert_diagnostic_contract(
        &diagnostic,
        SOURCE_ID,
        SOURCE,
        "compiler::unknown_constructor_field",
        "unknown constructor field `bonus` for `Reward`",
        "bonus",
        &[
            ExpectedLabel {
                span_text: "bonus",
                message: "field is not declared by the constructor schema",
            },
            ExpectedLabel {
                span_text: "bonus",
                message: "available fields: item_id, count",
            },
        ],
    );
}

#[test]
fn constructor_diagnostic_contracts_pin_variant_duplicate_and_missing_fields() {
    const VARIANT_SOURCE_ID: SourceId = SourceId::new(718);
    const VARIANT_SOURCE: &str = "enum Damage { Physical { amount: i64 } }\nfn main() { return Damage::Magical { amount: 7 }; }";
    let error = compile_program_source(VARIANT_SOURCE_ID, VARIANT_SOURCE)
        .expect_err("an unknown enum constructor variant should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);
    assert_diagnostic_contract(
        &diagnostic,
        VARIANT_SOURCE_ID,
        VARIANT_SOURCE,
        "compiler::unknown_constructor_variant",
        "unknown enum variant `Damage::Magical`",
        "Damage::Magical { amount: 7 }",
        &[ExpectedLabel {
            span_text: "Damage::Magical { amount: 7 }",
            message: "variant is not declared on this enum",
        }],
    );

    const DUPLICATE_SOURCE_ID: SourceId = SourceId::new(719);
    const DUPLICATE_SOURCE: &str = "struct Reward { item_id: String, count: i64 }\nfn main() { return Reward { item_id: \"gold\", item_id: \"xp\", count: 2 }; }";
    let error = compile_program_source(DUPLICATE_SOURCE_ID, DUPLICATE_SOURCE)
        .expect_err("a duplicate record constructor field should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);
    assert_diagnostic_contract_with_occurrences(
        &diagnostic,
        DUPLICATE_SOURCE_ID,
        DUPLICATE_SOURCE,
        "compiler::duplicate_constructor_field",
        "duplicate constructor field `item_id`",
        ExpectedSpanOccurrence {
            text: "item_id",
            occurrence: 2,
        },
        &[
            ExpectedLabelOccurrence {
                span_text: "item_id",
                occurrence: 1,
                message: "previous field is here",
            },
            ExpectedLabelOccurrence {
                span_text: "item_id",
                occurrence: 2,
                message: "duplicate field is here",
            },
        ],
    );

    const MISSING_SOURCE_ID: SourceId = SourceId::new(720);
    const MISSING_SOURCE: &str =
        "struct Reward { item_id: String, count: i64 = 1 }\nfn main() { return Reward {}; }";
    let error = compile_program_source(MISSING_SOURCE_ID, MISSING_SOURCE)
        .expect_err("an omitted required record constructor field should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);
    assert_diagnostic_contract(
        &diagnostic,
        MISSING_SOURCE_ID,
        MISSING_SOURCE,
        "compiler::missing_constructor_field",
        "missing constructor field `item_id` for `Reward`",
        "Reward {}",
        &[ExpectedLabel {
            span_text: "Reward {}",
            message: "required field is not provided and has no default",
        }],
    );
}

#[test]
fn pattern_diagnostic_contract_pins_repair_guidance() {
    const SOURCE_ID: SourceId = SourceId::new(709);
    const SOURCE: &str = "fn main(value) { return match value { (item) => item, _ => 0 }; }";
    let error = compile_program_source(SOURCE_ID, SOURCE)
        .expect_err("one-element tuple pattern should fail syntax validation");
    let diagnostic = only_syntax_diagnostic(error);

    assert_diagnostic_contract(
        &diagnostic,
        SOURCE_ID,
        SOURCE,
        "syntax::one_element_tuple_pattern",
        "one-element tuple patterns are not supported",
        "(item)",
        &[ExpectedLabel {
            span_text: "(item)",
            message: "use the element pattern directly or add another tuple element",
        }],
    );
}
