use super::*;
use vela_common::{Diagnostic, Severity};

#[derive(Clone, Copy)]
struct ExpectedLabel<'a> {
    text: &'a str,
    occurrence: usize,
    message: &'a str,
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

fn source_span(source_id: SourceId, source: &str, text: &str, occurrence: usize) -> Span {
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
    primary_text: &str,
    labels: &[ExpectedLabel<'_>],
) {
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.code.as_deref(), Some(code));
    assert_eq!(diagnostic.message, message);
    assert_eq!(
        diagnostic.span,
        Some(source_span(source_id, source, primary_text, 0))
    );
    assert_eq!(
        diagnostic
            .labels
            .iter()
            .map(|label| (label.span, label.message.as_str()))
            .collect::<Vec<_>>(),
        labels
            .iter()
            .map(|label| {
                (
                    source_span(source_id, source, label.text, label.occurrence),
                    label.message,
                )
            })
            .collect::<Vec<_>>()
    );
    assert!(diagnostic.candidates.is_empty());
    assert!(diagnostic.repairs.is_empty());
}

#[test]
fn loop_placement_failures_are_analysis_diagnostics_across_lambda_boundaries() {
    let cases = [
        (
            "fn main() { break; }",
            "analysis::break_outside_loop",
            "break outside loop",
            "break;",
            "root break",
        ),
        (
            "fn main() { continue; }",
            "analysis::continue_outside_loop",
            "continue outside loop",
            "continue;",
            "root continue",
        ),
        (
            "fn main() { for value in [1] { let stop = || { break; }; stop(); } }",
            "analysis::break_outside_loop",
            "break outside loop",
            "break;",
            "lambda break",
        ),
        (
            "fn main() { for value in [1] { let skip = || { continue; }; skip(); } }",
            "analysis::continue_outside_loop",
            "continue outside loop",
            "continue;",
            "lambda continue",
        ),
    ];

    for (source, code, message, statement, context) in cases {
        let error = match compile_program_source(SourceId::new(720), source) {
            Ok(_) => panic!("{context} should fail compilation"),
            Err(error) => error,
        };
        let diagnostic = only_semantic_diagnostic(error);
        assert_diagnostic_contract(
            &diagnostic,
            SourceId::new(720),
            source,
            code,
            message,
            statement,
            &[],
        );
    }
}

#[test]
fn nonconstant_schema_default_is_lazy_and_remains_uncoded_when_used() {
    const SOURCE_ID: SourceId = SourceId::new(721);
    const UNUSED: &str = r#"
struct Reward { amount: i64 = math::random() }
fn main() { return 1; }
"#;
    compile_program_source(SOURCE_ID, UNUSED)
        .expect("an unused nonconstant schema default is accepted by the current compiler");

    const USED: &str = r#"
struct Reward { amount: i64 = math::random() }
fn main() { return Reward {}; }
"#;
    let error = compile_program_source(SOURCE_ID, USED)
        .expect_err("using an omitted nonconstant schema default should fail");
    assert_eq!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("non-constant schema default expression")
    );
    assert_eq!(
        error.span,
        Some(source_span(SOURCE_ID, USED, "math::random()", 0))
    );
    assert_eq!(error.to_diagnostic(), None);
}

#[test]
fn known_receiver_missing_method_is_static_but_unknown_receiver_stays_dynamic() {
    const SOURCE_ID: SourceId = SourceId::new(722);
    const KNOWN: &str = "fn main() { return 42.trim(); }";
    let registry = vela_stdlib::standard_registry().expect("standard registry should build");
    let error = compile_program_source_with_registry(SOURCE_ID, KNOWN, registry.compile_view())
        .expect_err("a known integer receiver has no trim method");
    let diagnostic = only_semantic_diagnostic(error);
    assert_diagnostic_contract(
        &diagnostic,
        SOURCE_ID,
        KNOWN,
        "compiler::unresolved_method",
        "unresolved method `trim`",
        "42.trim()",
        &[ExpectedLabel {
            text: "42.trim()",
            occurrence: 0,
            message: "method is not defined for the known receiver type",
        }],
    );

    const UNKNOWN: &str = "fn main(value) { return value.trim(); }";
    let program = compile_program_source_with_registry(SOURCE_ID, UNKNOWN, registry.compile_view())
        .expect("an unknown receiver must retain dynamic method dispatch");
    let main = program.function("main").expect("main function");
    let dynamic_call = main
        .instructions
        .iter()
        .find(|instruction| {
            matches!(
                &instruction.kind,
                UnlinkedInstructionKind::CallDynamicMethod { method, args, .. }
                    if method == "trim" && args.is_empty()
            )
        })
        .expect("trim should lower to dynamic dispatch");
    assert_eq!(
        dynamic_call.span,
        Some(source_span(SOURCE_ID, UNKNOWN, "value.trim()", 0))
    );
}

#[test]
fn host_index_diagnostic_contracts_cover_every_access_mode_and_key_type() {
    const SOURCE_ID: SourceId = SourceId::new(723);

    assert_host_index_diagnostic(
        SOURCE_ID,
        "fn main(player: Player) { return player.inventory[\"gold\"]; }",
        None,
        "analysis::host_index_not_supported",
        "type `Inventory` does not support host index access",
        "player.inventory[\"gold\"]",
        &[
            ExpectedLabel {
                text: "player.inventory[\"gold\"]",
                occurrence: 0,
                message: "host index access is not registered for this type",
            },
            ExpectedLabel {
                text: "player.inventory",
                occurrence: 0,
                message: "register a host index capability or expose a field/method instead",
            },
        ],
    );

    for case in [
        HostIndexDiagnosticCase {
            source: "fn main(player: Player) { return player.inventory[\"gold\"]; }",
            capability: options::HostIndexCapabilityInfo::default(),
            code: "analysis::host_index_not_readable",
            message: "type `Inventory` does not allow host index reads",
            primary: "player.inventory[\"gold\"]",
            denial: "host index capability is not readable",
            enable: "enable readable host index access for this type",
        },
        HostIndexDiagnosticCase {
            source: "fn main(player: Player) { player.inventory[\"gold\"] = 1; return 0; }",
            capability: options::HostIndexCapabilityInfo::default(),
            code: "analysis::host_index_not_writable",
            message: "type `Inventory` does not allow host index writes",
            primary: "player.inventory[\"gold\"] = 1",
            denial: "host index capability is not writable",
            enable: "enable writable host index access for this type",
        },
        HostIndexDiagnosticCase {
            source: "fn main(player: Player) { player.inventory[\"gold\"] += 1; return 0; }",
            capability: options::HostIndexCapabilityInfo::default(),
            code: "analysis::host_index_not_mutable",
            message: "type `Inventory` does not allow host index mutations",
            primary: "player.inventory[\"gold\"] += 1",
            denial: "host index capability is not addable",
            enable: "enable addable host index access for this type",
        },
        HostIndexDiagnosticCase {
            source: "fn main(player: Player) { player.inventory[\"gold\"].remove(); return 0; }",
            capability: options::HostIndexCapabilityInfo::default(),
            code: "analysis::host_index_not_removable",
            message: "type `Inventory` does not allow host index removals",
            primary: "player.inventory[\"gold\"].remove()",
            denial: "host index capability is not removable",
            enable: "enable removable host index access for this type",
        },
    ] {
        assert_host_index_diagnostic(
            SOURCE_ID,
            case.source,
            Some(case.capability),
            case.code,
            case.message,
            case.primary,
            &[
                ExpectedLabel {
                    text: case.primary,
                    occurrence: 0,
                    message: case.denial,
                },
                ExpectedLabel {
                    text: "player.inventory",
                    occurrence: 0,
                    message: case.enable,
                },
            ],
        );
    }

    assert_host_index_diagnostic(
        SOURCE_ID,
        "fn main(player: Player) { return player.inventory[\"gold\"]; }",
        Some(options::HostIndexCapabilityInfo {
            readable: true,
            key_type: Some("i64".to_owned()),
            ..Default::default()
        }),
        "analysis::host_index_key_mismatch",
        "host index key for `Inventory` must be `i64`",
        "player.inventory[\"gold\"]",
        &[ExpectedLabel {
            text: "\"gold\"",
            occurrence: 0,
            message: "index expression has type `String`",
        }],
    );
}

struct HostIndexDiagnosticCase {
    source: &'static str,
    capability: options::HostIndexCapabilityInfo,
    code: &'static str,
    message: &'static str,
    primary: &'static str,
    denial: &'static str,
    enable: &'static str,
}

fn assert_host_index_diagnostic(
    source_id: SourceId,
    source: &str,
    capability: Option<options::HostIndexCapabilityInfo>,
    code: &str,
    message: &str,
    primary: &str,
    labels: &[ExpectedLabel<'_>],
) {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "Player",
            ))
            .host_runtime_id(722),
        )
        .expect("Player type should register");
    registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "Inventory",
            ))
            .host_runtime_id(723),
        )
        .expect("Inventory type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                vela_def::DefPath::field("host", std::iter::empty::<&str>(), "Player", "inventory"),
                player,
            )
            .type_hint(Some("Inventory"))
            .host_runtime_id(724),
        )
        .expect("Player::inventory field should register");
    let options = capability.map_or_else(options::CompilerOptions::new, |capability| {
        options::CompilerOptions::new().with_host_index_capability("Inventory", capability)
    });
    let error = compile_program_source_with_options_and_registry(
        source_id,
        source,
        &options,
        registry.compile_view(),
    )
    .expect_err("host index fixture should fail compilation");
    let diagnostic = only_semantic_diagnostic(error);
    assert_diagnostic_contract(
        &diagnostic,
        source_id,
        source,
        code,
        message,
        primary,
        labels,
    );
}

#[test]
fn negated_float_const_and_schema_default_keep_literal_origins() {
    const SOURCE_ID: SourceId = SourceId::new(724);
    for source in [
        "const BAD = -3.5e38f32; fn main() { return BAD; }",
        "struct Reward { amount: f32 = -3.5e38f32 } fn main() { return Reward {}; }",
    ] {
        let error = compile_program_source(SOURCE_ID, source)
            .expect_err("out-of-range negated f32 should fail compilation");
        assert_eq!(
            error.kind,
            CompileErrorKind::InvalidFloatLiteral {
                literal: "3.5e38f32".to_owned(),
                error: "float literal out of range".to_owned(),
            }
        );
        let diagnostic = error
            .to_diagnostic()
            .expect("invalid float literals should project to a diagnostic");
        assert_diagnostic_contract(
            &diagnostic,
            SOURCE_ID,
            source,
            "compiler::invalid_float_literal",
            "invalid float literal `3.5e38f32`: float literal out of range",
            "3.5e38f32",
            &[],
        );
    }
}
