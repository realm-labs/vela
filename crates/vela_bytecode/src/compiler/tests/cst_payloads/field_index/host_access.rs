use crate::UnlinkedInstructionKind;
use crate::compiler::assignments::{
    AssignmentTargetSyntax, AssignmentValuePayloads, AssignmentValueSyntax,
};
use crate::compiler::host_paths::HostIndexAccessKind;
use crate::compiler::options::HostIndexCapabilityInfo;
use crate::compiler::tests::cst_payloads::{
    cst_payload_compiler_facts_with_options, global_slots, parse_semantic_source,
    semantic_diagnostic_codes,
};
use crate::compiler::tests::{
    expression_payload_with_fallback, paired_statement_payloads_for_body,
};
use crate::compiler::{CompileErrorKind, Compiler, CompilerFacts, CompilerOptions};
use vela_common::SourceId;
use vela_syntax::ast::{ExprKind, SyntaxExpressionKind};

#[test]
fn host_field_path_with_non_field_cst_payload_does_not_use_legacy_field_name() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "CstHost",
            ))
            .host_runtime_id(77),
        )
        .expect("CstHost host type should register");
    let legacy = registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "LegacyHost",
            ))
            .host_runtime_id(78),
        )
        .expect("LegacyHost host type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                vela_def::DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "LegacyHost",
                    "amount",
                ),
                legacy,
            )
            .host_runtime_id(vela_def::FieldId::new(4).get())
            .writable(true)
            .type_hint(Some("i64".to_owned())),
        )
        .expect("LegacyHost amount field should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(cst: CstHost, legacy: LegacyHost) {
    let cst_value = {
        let selected = cst;
        selected;
        selected && true
    };
    let legacy_value = make(legacy).amount;
}

fn make(value) {
    return value;
}
"#,
    )
    .expect("semantic source should parse");
    let facts = cst_payload_compiler_facts_with_options(
        &semantic,
        CompilerOptions::default(),
        Some(registry.compile_view()),
    );
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let statements = paired_statement_payloads_for_body(source, &payload.body);
    let cst_block = statements[0]
        .let_initializer_expression_payload()
        .expect("CST block initializer");
    let legacy_field = statements[1]
        .let_initializer_expression_payload()
        .expect("legacy host field fallback");
    assert_eq!(cst_block.syntax_kind(), Some(SyntaxExpressionKind::Block));
    let mismatched_payload = expression_payload_with_fallback(
        source,
        cst_block
            .syntax_expression()
            .expect("CST block syntax")
            .clone(),
        legacy_field.fallback(),
    );
    let compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    assert!(
        compiler
            .resolve_host_path_with_payload(
                mismatched_payload.fallback(),
                Some(&mismatched_payload)
            )
            .is_none(),
        "non-field CST payload should not resolve the legacy host field path"
    );
}

#[test]
fn host_index_validation_prefers_cst_receiver_payloads() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "CstMap",
            ))
            .host_runtime_id(77),
        )
        .expect("CstMap host type should register");
    registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "LegacyMap",
            ))
            .host_runtime_id(78),
        )
        .expect("LegacyMap host type should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(cst: CstMap, legacy: LegacyMap) {
    let cst_value = cst[1];
    let legacy_value = legacy[false];
}
"#,
    )
    .expect("semantic source should parse");
    let script_function_symbols = semantic.script_function_symbols();
    let script_function_signatures = semantic.script_function_signatures();
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let global_slots = global_slots(&global_symbols);
    let global_type_symbols = semantic.global_type_symbols();
    let script_field_slots = semantic.script_field_slots(&type_symbols);
    let const_values = semantic.const_values().expect("const values should lower");
    let schema_defaults = semantic.schema_defaults(&type_symbols, &const_values);
    let facts = CompilerFacts {
        script_function_symbols,
        script_function_signatures,
        script_method_ids: std::collections::BTreeMap::new(),
        script_method_signatures: std::collections::BTreeMap::new(),
        derived_operator_traits: std::collections::BTreeMap::new(),
        script_field_slots,
        schema_defaults,
        type_symbols,
        global_symbols,
        global_slots,
        global_type_symbols,
        const_values,
        options: CompilerOptions::new()
            .with_host_index_capability(
                "CstMap",
                HostIndexCapabilityInfo {
                    readable: true,
                    writable: true,
                    addable: true,
                    removable: false,
                    key_type: Some("i64".to_owned()),
                    value_type: Some("i64".to_owned()),
                },
            )
            .with_host_index_capability(
                "LegacyMap",
                HostIndexCapabilityInfo {
                    readable: true,
                    writable: true,
                    addable: true,
                    removable: true,
                    key_type: Some("bool".to_owned()),
                    value_type: Some("i64".to_owned()),
                },
            ),
        registry: Some(registry.compile_view()),
    };
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let statements = paired_statement_payloads_for_body(source, &payload.body);
    let cst_index = statements[0]
        .let_initializer_expression_payload()
        .expect("CST index initializer");
    let legacy_index = statements[1]
        .let_initializer_expression_payload()
        .expect("legacy index initializer");
    let mismatched_index = expression_payload_with_fallback(
        source,
        cst_index
            .syntax_expression()
            .expect("CST index syntax")
            .clone(),
        legacy_index.fallback(),
    );
    let (base_payload, index_payload) = mismatched_index
        .index_operand_payloads()
        .expect("mismatched index payloads");
    let ExprKind::Index { base, index } = &mismatched_index.fallback().kind else {
        panic!("expected legacy index fallback");
    };
    let mut compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    compiler
        .reject_invalid_host_index_read_with_payload(
            mismatched_index.fallback(),
            base,
            index,
            Some(&base_payload),
            Some(&index_payload),
        )
        .expect("CST receiver payload should select CstMap key contract");
    compiler
        .reject_terminal_host_index_access(
            mismatched_index.fallback(),
            None,
            HostIndexAccessKind::Remove,
        )
        .expect("legacy receiver should allow removable host index");
    let cst_remove_error = compiler
        .reject_terminal_host_index_access(
            mismatched_index.fallback(),
            Some(&mismatched_index),
            HostIndexAccessKind::Remove,
        )
        .expect_err("CST receiver should reject non-removable host index");
    assert_eq!(
        semantic_diagnostic_codes(cst_remove_error),
        ["analysis::host_index_not_removable"]
    );
    let register = compiler
        .compile_expr_with_payload(mismatched_index.fallback(), Some(&mismatched_index))
        .expect("mismatched fallback should not block CST host index compilation");
    assert!(
        compiler
            .code
            .instructions
            .iter()
            .any(|instruction| matches!(
                instruction.kind,
                UnlinkedInstructionKind::HostRead {
                    dst,
                    ref dynamic_args,
                    ..
                } if dst == register && dynamic_args.len() == 1
            ))
    );
}

#[test]
fn read_only_host_assignment_prefers_cst_target_payloads() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let readonly = registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "ReadOnlyHost",
            ))
            .host_runtime_id(77),
        )
        .expect("ReadOnlyHost host type should register");
    let writable = registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "WritableHost",
            ))
            .host_runtime_id(78),
        )
        .expect("WritableHost host type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                vela_def::DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "ReadOnlyHost",
                    "amount",
                ),
                readonly,
            )
            .host_runtime_id(vela_def::FieldId::new(3).get())
            .writable(false),
        )
        .expect("ReadOnlyHost amount field should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                vela_def::DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "WritableHost",
                    "amount",
                ),
                writable,
            )
            .host_runtime_id(vela_def::FieldId::new(4).get())
            .writable(true),
        )
        .expect("WritableHost amount field should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(readonly: ReadOnlyHost, writable: WritableHost) {
    readonly.amount = 1;
    writable.amount = 2;
}
"#,
    )
    .expect("semantic source should parse");
    let script_function_symbols = semantic.script_function_symbols();
    let script_function_signatures = semantic.script_function_signatures();
    let type_symbols = semantic.type_symbols();
    let global_symbols = semantic.global_symbols();
    let global_slots = global_slots(&global_symbols);
    let global_type_symbols = semantic.global_type_symbols();
    let script_field_slots = semantic.script_field_slots(&type_symbols);
    let const_values = semantic.const_values().expect("const values should lower");
    let schema_defaults = semantic.schema_defaults(&type_symbols, &const_values);
    let facts = CompilerFacts {
        script_function_symbols,
        script_function_signatures,
        script_method_ids: std::collections::BTreeMap::new(),
        script_method_signatures: std::collections::BTreeMap::new(),
        derived_operator_traits: std::collections::BTreeMap::new(),
        script_field_slots,
        schema_defaults,
        type_symbols,
        global_symbols,
        global_slots,
        global_type_symbols,
        const_values,
        options: CompilerOptions::default(),
        registry: Some(registry.compile_view()),
    };
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let statements = paired_statement_payloads_for_body(source, &payload.body);
    let readonly_target = statements[0]
        .expression_payload()
        .and_then(|payload| payload.assignment_target_payload())
        .expect("CST read-only assignment target");
    let writable_target = statements[1]
        .expression_payload()
        .and_then(|payload| payload.assignment_target_payload())
        .expect("legacy writable assignment target");
    let writable_statement = statements[1]
        .expression_payload()
        .expect("legacy writable assignment expression");
    let mismatched_target = expression_payload_with_fallback(
        source,
        readonly_target
            .syntax_expression()
            .expect("CST read-only target syntax")
            .clone(),
        writable_target.fallback(),
    );
    let mut compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");
    let receiver_payload = mismatched_target
        .field_base_payload()
        .expect("mismatched target should retain CST receiver payload");
    assert_eq!(
        compiler
            .script_type_for_payload(&receiver_payload)
            .as_deref(),
        Some("ReadOnlyHost")
    );

    let error = compiler
        .compile_assignment_with_payloads(
            writable_statement.fallback(),
            AssignmentTargetSyntax::new(Some(&mismatched_target)),
            AssignmentValueSyntax::new(None, None, None, AssignmentValuePayloads::new(None)),
        )
        .expect_err("mismatched CST read-only assignment target must not compile");
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST assignment target")
    ));
}

#[test]
fn read_only_host_assignment_with_non_field_cst_payload_does_not_use_legacy_field() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let readonly = registry
        .register_type(
            vela_registry::TypeDef::new(vela_def::DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "ReadOnlyHost",
            ))
            .host_runtime_id(77),
        )
        .expect("ReadOnlyHost host type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                vela_def::DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "ReadOnlyHost",
                    "amount",
                ),
                readonly,
            )
            .host_runtime_id(vela_def::FieldId::new(3).get())
            .writable(false),
        )
        .expect("ReadOnlyHost amount field should register");

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(readonly: ReadOnlyHost) {
    let cst_target = {
        let selected = readonly;
        selected;
        selected && true
    };
    readonly.amount = 1;
}
"#,
    )
    .expect("semantic source should parse");
    let facts = cst_payload_compiler_facts_with_options(
        &semantic,
        CompilerOptions::default(),
        Some(registry.compile_view()),
    );
    let (payload, signature, bindings) = semantic.function("main").expect("main function");
    let statements = paired_statement_payloads_for_body(source, &payload.body);
    let cst_block = statements[0]
        .let_initializer_expression_payload()
        .expect("CST block initializer");
    let legacy_target = statements[1]
        .expression_payload()
        .and_then(|payload| payload.assignment_target_payload())
        .expect("legacy read-only assignment target");
    let legacy_statement = statements[1]
        .expression_payload()
        .expect("legacy read-only assignment expression");
    let mismatched_target = expression_payload_with_fallback(
        source,
        cst_block
            .syntax_expression()
            .expect("CST block syntax")
            .clone(),
        legacy_target.fallback(),
    );
    let mut compiler = Compiler::new_with_param_defaults(
        payload.name.clone(),
        payload.body.clone(),
        payload.param_defaults.clone(),
        signature,
        bindings,
        facts,
    )
    .expect("compiler should initialize");

    let error = compiler
        .compile_assignment_with_payloads(
            legacy_statement.fallback(),
            AssignmentTargetSyntax::new(Some(&mismatched_target)),
            AssignmentValueSyntax::new(None, None, None, AssignmentValuePayloads::new(None)),
        )
        .expect_err("mismatched non-field CST target should be rejected");
    assert!(
        matches!(error.kind, CompileErrorKind::UnsupportedSyntax(message) if message == "mismatched CST assignment target"),
        "expected mismatched assignment target, got {:?}",
        error.kind
    );
}
