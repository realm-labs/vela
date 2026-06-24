use super::*;
use crate::HostTargetPlanId;
use crate::compiler::options::HostIndexCapabilityInfo;
use vela_common::HostTypeId;
use vela_def::{DefPath, TypeId};
use vela_host::target::HostPathPart;

mod root_indexes;

fn register_registry_host_type(
    registry: &mut vela_registry::DefinitionRegistry,
    name: &str,
    runtime_id: HostTypeId,
) -> TypeId {
    registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty("host", std::iter::empty::<&str>(), name))
                .host_runtime_id(runtime_id.get().into()),
        )
        .expect("test host type should register")
}

fn register_registry_host_field(
    registry: &mut vela_registry::DefinitionRegistry,
    owner: TypeId,
    owner_name: &str,
    name: &str,
    id: FieldId,
    writable: bool,
) {
    register_registry_host_field_with_type(registry, owner, owner_name, name, id, writable, None);
}

fn register_registry_host_field_with_type(
    registry: &mut vela_registry::DefinitionRegistry,
    owner: TypeId,
    owner_name: &str,
    name: &str,
    id: FieldId,
    writable: bool,
    type_hint: Option<&str>,
) {
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), owner_name, name),
                owner,
            )
            .host_runtime_id(id.get())
            .writable(writable)
            .type_hint(type_hint.map(str::to_owned)),
        )
        .expect("test host field should register");
}

fn register_registry_host_variant_field(
    registry: &mut vela_registry::DefinitionRegistry,
    owner: TypeId,
    owner_name: &str,
    name: &str,
    id: FieldId,
) {
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), owner_name, name),
                owner,
            )
            .host_runtime_id(id.get())
            .variant_field(true),
        )
        .expect("test host variant field should register");
}

fn register_registry_host_method(
    registry: &mut vela_registry::DefinitionRegistry,
    owner: TypeId,
    owner_name: &str,
    name: &str,
    runtime_id: HostMethodId,
    params: impl IntoIterator<Item = vela_registry::ParamDef>,
) {
    registry
        .register_method(
            vela_registry::MethodDef::new(
                DefPath::method("host", std::iter::empty::<&str>(), owner_name, name),
                owner,
                vela_registry::FunctionSignature::new(params, None::<vela_registry::TypeHintDef>),
            )
            .host_runtime_id(runtime_id.get()),
        )
        .expect("test host method should register");
}

fn host_target_parts(code: &UnlinkedCodeObject, target: HostTargetPlanId) -> &[HostPathPart] {
    code.host_target(target)
        .expect("host target should exist")
        .parts
        .as_slice()
}

fn has_host_call(code: &UnlinkedCodeObject, method: HostMethodId, arg_count: usize) -> bool {
    code.instructions.iter().any(|instruction| {
        matches!(
            &instruction.kind,
            UnlinkedInstructionKind::HostCall {
                method: lowered_method,
                args,
                ..
            } if *lowered_method == method && args.len() == arg_count
        )
    })
}

fn has_host_call_target(
    code: &UnlinkedCodeObject,
    method: HostMethodId,
    expected: &[HostPathPart],
    dynamic_arg_count: usize,
) -> bool {
    code.instructions
        .iter()
        .any(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::HostCall {
                method: lowered_method,
                target,
                dynamic_args,
                ..
            } => {
                *lowered_method == method
                    && dynamic_args.len() == dynamic_arg_count
                    && host_target_parts(code, *target) == expected
            }
            _ => false,
        })
}

fn has_host_mutate_target(
    code: &UnlinkedCodeObject,
    op: vela_host::resolved::HostMutationOp,
    expected: &[HostPathPart],
    dynamic_arg_count: usize,
) -> bool {
    code.instructions
        .iter()
        .any(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::HostMutate {
                op: lowered_op,
                target,
                dynamic_args,
                ..
            } => {
                *lowered_op == op
                    && dynamic_args.len() == dynamic_arg_count
                    && host_target_parts(code, *target) == expected
            }
            _ => false,
        })
}

fn has_host_read_target(
    code: &UnlinkedCodeObject,
    expected: &[HostPathPart],
    dynamic_arg_count: usize,
) -> bool {
    code.instructions
        .iter()
        .any(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::HostRead {
                target,
                dynamic_args,
                ..
            } => {
                dynamic_args.len() == dynamic_arg_count
                    && host_target_parts(code, *target) == expected
            }
            _ => false,
        })
}

fn has_host_write_target(
    code: &UnlinkedCodeObject,
    expected: &[HostPathPart],
    dynamic_arg_count: usize,
) -> bool {
    code.instructions
        .iter()
        .any(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::HostWrite {
                target,
                dynamic_args,
                ..
            } => {
                dynamic_args.len() == dynamic_arg_count
                    && host_target_parts(code, *target) == expected
            }
            _ => false,
        })
}

#[test]
fn compiler_lowers_typed_host_target_root_type_id() {
    let player_type = HostTypeId::new(77);
    let level = FieldId::new(3);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", player_type);
    register_registry_host_field(&mut registry, player, "Player", "level", level, true);
    let code = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    return player.level;
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect("typed host field read should compile");

    let Some(target) = code
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            UnlinkedInstructionKind::HostRead { target, .. } => Some(target),
            _ => None,
        })
    else {
        panic!("expected HostRead");
    };
    let plan = code.host_target(target).expect("host target should exist");
    assert_eq!(plan.root_type, player_type);
    assert_eq!(plan.parts.as_slice(), [HostPathPart::Field(level)]);
}

#[test]
fn cst_host_path_receivers_drive_root_type_and_field_lookup() {
    let cst_type = HostTypeId::new(77);
    let legacy_type = HostTypeId::new(78);
    let cst_amount = FieldId::new(3);
    let legacy_amount = FieldId::new(4);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let cst = register_registry_host_type(&mut registry, "CstHost", cst_type);
    let legacy = register_registry_host_type(&mut registry, "LegacyHost", legacy_type);
    register_registry_host_field_with_type(
        &mut registry,
        cst,
        "CstHost",
        "amount",
        cst_amount,
        true,
        Some("i64"),
    );
    register_registry_host_field_with_type(
        &mut registry,
        legacy,
        "LegacyHost",
        "amount",
        legacy_amount,
        true,
        Some("bool"),
    );

    let source = SourceId::new(1);
    let semantic = parse_semantic_source(
        source,
        r#"
fn main(cst: CstHost, legacy: LegacyHost) {
    cst.amount = 1;
    legacy.amount = 2;
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
    let derived_operator_traits =
        derived_operator_traits(&semantic.script_metadata_graph(), &type_symbols);
    let const_values = semantic.const_values().expect("const values should lower");
    let schema_defaults = semantic.schema_defaults(&type_symbols, &const_values);
    let facts = CompilerFacts {
        script_function_symbols,
        script_function_signatures,
        script_method_ids: std::collections::BTreeMap::new(),
        script_method_signatures: std::collections::BTreeMap::new(),
        derived_operator_traits,
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
    let statements = payload.body.statement_payloads();
    let cst_target = statements[0]
        .assignment_target_expression_payload()
        .expect("CST assignment target");
    let legacy_target = statements[1]
        .assignment_target_expression_payload()
        .expect("legacy assignment target");
    let legacy_statement = statements[1]
        .expression_payload()
        .expect("legacy assignment expression");
    let mismatched_target = body_payloads::CompilerExpressionPayload::syntax(
        source,
        cst_target
            .syntax_expression()
            .expect("CST target syntax")
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

    let resolved = compiler
        .resolve_host_path_with_payload(mismatched_target.fallback(), Some(&mismatched_target))
        .expect("CST-backed host path should resolve");
    assert_eq!(resolved.type_name.as_deref(), Some("i64"));
    let error = compiler
        .compile_expr_with_payload(mismatched_target.fallback(), Some(&mismatched_target))
        .expect_err("mismatched CST host read payload must not compile");
    assert!(matches!(
        error.kind,
        CompileErrorKind::UnsupportedSyntax("mismatched CST field expression payload")
    ));
    compiler
        .compile_assignment_with_payloads(
            legacy_statement.fallback(),
            crate::compiler::assignments::AssignmentTargetSyntax::new(Some(&mismatched_target)),
            crate::compiler::assignments::AssignmentValueSyntax::new(
                None,
                None,
                crate::compiler::assignments::AssignmentValuePayloads::new(None, None, None, None),
            ),
        )
        .expect("CST-backed host write should compile");
    let target = compiler
        .code
        .instructions
        .iter()
        .rev()
        .find_map(|instruction| match instruction.kind {
            UnlinkedInstructionKind::HostWrite { target, .. } => Some(target),
            _ => None,
        })
        .expect("host write should be emitted");
    let plan = compiler
        .code
        .host_target(target)
        .expect("host target should exist");
    assert_eq!(plan.root_type, cst_type);
    assert_eq!(plan.parts.as_slice(), [HostPathPart::Field(cst_amount)]);
}

#[test]
fn compiler_lowers_host_field_reads_from_registry() {
    let player_type = HostTypeId::new(77);
    let level = FieldId::new(3);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", player_type);
    register_registry_host_field(&mut registry, player, "Player", "level", level, true);

    let code = compile_function_source_with_options_and_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    return player.level;
}
"#,
        "main",
        &CompilerOptions::new(),
        registry.compile_view(),
    )
    .expect("registry host field read should compile");

    let Some(target) = code
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            UnlinkedInstructionKind::HostRead { target, .. } => Some(target),
            _ => None,
        })
    else {
        panic!("expected HostRead");
    };
    let plan = code.host_target(target).expect("host target should exist");
    assert_eq!(plan.root_type, player_type);
    assert_eq!(plan.parts.as_slice(), [HostPathPart::Field(level)]);
}

#[test]
fn compiler_rejects_read_only_host_field_assignment_from_registry() {
    let id = FieldId::new(3);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", HostTypeId::new(77));
    register_registry_host_field(&mut registry, player, "Player", "id", id, false);

    let error = compile_function_source_with_options_and_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.id = 8;
    return player.id;
}
"#,
        "main",
        &CompilerOptions::new(),
        registry.compile_view(),
    )
    .expect_err("read-only registry host field assignment should be rejected");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::field_not_writable"]
    );
}

#[test]
fn compiler_lowers_configured_host_method_calls() {
    let method = HostMethodId::new(5);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", HostTypeId::new(77));
    register_registry_host_method(
        &mut registry,
        player,
        "Player",
        "grant_exp",
        method,
        [vela_registry::ParamDef::new("amount", Some("i64"))],
    );
    let code = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.grant_exp(20);
    return 1;
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect("host method call should compile");
    assert!(has_host_call(&code, method, 1));
}

#[test]
fn compiler_lowers_host_method_calls_and_args_from_registry() {
    let method = HostMethodId::new(5);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", HostTypeId::new(77));
    register_registry_host_method(
        &mut registry,
        player,
        "Player",
        "grant_exp",
        method,
        [
            vela_registry::ParamDef::new("amount", Some("i64")),
            vela_registry::ParamDef::new("reason", Some("string")).defaulted(true),
        ],
    );

    let code = compile_function_source_with_options_and_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.grant_exp(amount = 20);
    return 1;
}
"#,
        "main",
        &CompilerOptions::new(),
        registry.compile_view(),
    )
    .expect("registry host method call should compile");

    assert!(has_host_call(&code, method, 1));
}

#[test]
fn compiler_lowers_named_and_default_host_method_args_from_registry() {
    let method = HostMethodId::new(5);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let ctx = register_registry_host_type(&mut registry, "Ctx", HostTypeId::new(81));
    register_registry_host_method(
        &mut registry,
        ctx,
        "Ctx",
        "emit",
        method,
        [
            vela_registry::ParamDef::new("event", Some("string")),
            vela_registry::ParamDef::new("payload", Some("any")).defaulted(true),
        ],
    );
    let code = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(ctx: Ctx) {
    ctx.emit(event = "player.level_checked");
    return 1;
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect("named/default host method args should compile");

    assert!(has_host_call(&code, method, 1));
}

#[test]
fn compiler_keeps_positional_host_method_args_variadic_with_metadata() {
    let method = HostMethodId::new(5);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let ctx = register_registry_host_type(&mut registry, "Ctx", HostTypeId::new(81));
    register_registry_host_method(
        &mut registry,
        ctx,
        "Ctx",
        "emit",
        method,
        [
            vela_registry::ParamDef::new("event", Some("string")),
            vela_registry::ParamDef::new("payload", Some("any")).defaulted(true),
        ],
    );
    let code = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(ctx: Ctx) {
    ctx.emit("player.level_checked", 10, 42);
    return 1;
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect("positional host method args should stay variadic");

    assert!(has_host_call(&code, method, 3));
}

#[test]
fn compiler_reports_named_host_method_arg_diagnostics_from_registry() {
    let method = HostMethodId::new(5);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let ctx = register_registry_host_type(&mut registry, "Ctx", HostTypeId::new(81));
    register_registry_host_method(
        &mut registry,
        ctx,
        "Ctx",
        "emit",
        method,
        [
            vela_registry::ParamDef::new("event", Some("string")),
            vela_registry::ParamDef::new("payload", Some("any")).defaulted(true),
        ],
    );
    let error = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(ctx: Ctx) {
    ctx.emit(evnt = "player.level_checked");
    return 1;
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect_err("unknown named host method arg should fail");

    assert_eq!(
        semantic_diagnostic_codes(error),
        [
            "compiler::unknown_named_argument",
            "compiler::missing_required_argument"
        ]
    );
}

#[test]
fn compiler_lowers_local_host_method_when_root_matches_native_module() {
    let method = HostMethodId::new(5);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let ctx = register_registry_host_type(&mut registry, "Ctx", HostTypeId::new(81));
    register_registry_host_method(
        &mut registry,
        ctx,
        "Ctx",
        "emit",
        method,
        [vela_registry::ParamDef::new("event", Some("string"))],
    );
    let code = compile_function_source_with_options_and_registry(
        SourceId::new(1),
        r#"
fn main(ctx: Ctx) {
    ctx.emit("player.level_checked");
    return 1;
}
"#,
        "main",
        &CompilerOptions::new().with_native_module_root("ctx"),
        registry.compile_view(),
    )
    .expect("local host method should shadow native module root");
    assert!(has_host_call(&code, method, 1));
}

#[test]
fn compiler_lowers_configured_host_method_calls_on_field_paths() {
    let inventory = FieldId::new(3);
    let method = HostMethodId::new(5);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", HostTypeId::new(77));
    let inventory_ty = register_registry_host_type(&mut registry, "Inventory", HostTypeId::new(78));
    register_registry_host_field_with_type(
        &mut registry,
        player,
        "Player",
        "inventory",
        inventory,
        true,
        Some("Inventory"),
    );
    register_registry_host_method(
        &mut registry,
        inventory_ty,
        "Inventory",
        "add",
        method,
        [
            vela_registry::ParamDef::new("kind", Some("string")),
            vela_registry::ParamDef::new("amount", Some("i64")),
        ],
    );
    let code = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.inventory.add("gold", 20);
    return 1;
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect("host field method call should compile");
    assert!(has_host_call_target(
        &code,
        method,
        &[HostPathPart::Field(inventory)],
        0
    ));
}
#[test]
fn compiler_lowers_configured_host_method_calls_on_indexed_paths() {
    let inventory = FieldId::new(3);
    let items = FieldId::new(4);
    let method = HostMethodId::new(5);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", HostTypeId::new(77));
    let inventory_ty = register_registry_host_type(&mut registry, "Inventory", HostTypeId::new(78));
    let item_ty = register_registry_host_type(&mut registry, "Item", HostTypeId::new(79));
    register_registry_host_field_with_type(
        &mut registry,
        player,
        "Player",
        "inventory",
        inventory,
        true,
        Some("Inventory"),
    );
    register_registry_host_field_with_type(
        &mut registry,
        inventory_ty,
        "Inventory",
        "items",
        items,
        true,
        Some("ItemMap"),
    );
    register_registry_host_method(
        &mut registry,
        item_ty,
        "Item",
        "grant",
        method,
        [vela_registry::ParamDef::new("amount", Some("i64"))],
    );
    let code = compile_function_source_with_options_and_registry(
        SourceId::new(1),
        r#"
fn main(player: Player, item_id) {
    player.inventory.items[item_id].grant(20);
    return 1;
}
"#,
        "main",
        &CompilerOptions::new().with_host_index_capability(
            "ItemMap",
            HostIndexCapabilityInfo {
                readable: true,
                writable: true,
                addable: true,
                removable: true,
                key_type: None,
                value_type: Some("Item".to_owned()),
            },
        ),
        registry.compile_view(),
    )
    .expect("indexed host method call should compile");
    assert!(has_host_call_target(
        &code,
        method,
        &[
            HostPathPart::Field(inventory),
            HostPathPart::Field(items),
            HostPathPart::DynKey { arg: 0 },
        ],
        1
    ));
}
#[test]
fn compiler_lowers_nested_host_field_paths() {
    let stats = FieldId::new(3);
    let level = FieldId::new(4);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", HostTypeId::new(77));
    let stats_ty = register_registry_host_type(&mut registry, "Stats", HostTypeId::new(78));
    register_registry_host_field_with_type(
        &mut registry,
        player,
        "Player",
        "stats",
        stats,
        true,
        Some("Stats"),
    );
    register_registry_host_field(&mut registry, stats_ty, "Stats", "level", level, true);
    let code = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.stats.level += 2;
    return player.stats.level;
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect("nested host field path should compile");
    let target = [HostPathPart::Field(stats), HostPathPart::Field(level)];
    assert!(has_host_mutate_target(
        &code,
        vela_host::resolved::HostMutationOp::Add,
        &target,
        0
    ));
    assert!(has_host_read_target(&code, &target, 0));
}

#[test]
fn compiler_rejects_read_only_host_field_assignment_for_typed_receiver() {
    let id = FieldId::new(3);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", HostTypeId::new(77));
    register_registry_host_field(&mut registry, player, "Player", "id", id, false);
    let error = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.id = 8;
    return player.id;
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect_err("read-only host field assignment should be rejected");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::field_not_writable"]
    );
}

#[test]
fn compiler_lowers_indexed_host_field_paths() {
    let inventory = FieldId::new(3);
    let items = FieldId::new(4);
    let count = FieldId::new(5);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", HostTypeId::new(77));
    let inventory_ty = register_registry_host_type(&mut registry, "Inventory", HostTypeId::new(78));
    let item_ty = register_registry_host_type(&mut registry, "Item", HostTypeId::new(79));
    register_registry_host_field_with_type(
        &mut registry,
        player,
        "Player",
        "inventory",
        inventory,
        true,
        Some("Inventory"),
    );
    register_registry_host_field_with_type(
        &mut registry,
        inventory_ty,
        "Inventory",
        "items",
        items,
        true,
        Some("ItemMap"),
    );
    register_registry_host_field(&mut registry, item_ty, "Item", "count", count, true);
    let code = compile_function_source_with_options_and_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.inventory.items[{
        let item_id = "gold";
        item_id
    }].count += 1;
    return player.inventory.items[{
        let item_id = "gold";
        item_id
    }].count;
}
"#,
        "main",
        &CompilerOptions::new().with_host_index_capability(
            "ItemMap",
            HostIndexCapabilityInfo {
                readable: true,
                writable: true,
                addable: true,
                removable: true,
                key_type: None,
                value_type: Some("Item".to_owned()),
            },
        ),
        registry.compile_view(),
    )
    .expect("indexed host field path should compile");
    let target = [
        HostPathPart::Field(inventory),
        HostPathPart::Field(items),
        HostPathPart::DynKey { arg: 0 },
        HostPathPart::Field(count),
    ];
    assert!(has_host_mutate_target(
        &code,
        vela_host::resolved::HostMutationOp::Add,
        &target,
        1
    ));
    assert!(has_host_read_target(&code, &target, 1));
}

#[test]
fn compiler_lowers_host_variant_field_paths() {
    let quest_progress = FieldId::new(3);
    let count = FieldId::new(4);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", HostTypeId::new(77));
    let quest_ty = register_registry_host_type(&mut registry, "QuestProgress", HostTypeId::new(78));
    register_registry_host_field_with_type(
        &mut registry,
        player,
        "Player",
        "quest_progress",
        quest_progress,
        true,
        Some("QuestProgress"),
    );
    register_registry_host_variant_field(&mut registry, quest_ty, "QuestProgress", "count", count);
    let code = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.quest_progress.count += 1;
    return player.quest_progress.count;
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect("host variant field path should compile");
    let target = [
        HostPathPart::Field(quest_progress),
        HostPathPart::VariantField(count),
    ];
    assert!(has_host_mutate_target(
        &code,
        vela_host::resolved::HostMutationOp::Add,
        &target,
        0
    ));
    assert!(has_host_read_target(&code, &target, 0));
}
#[test]
fn compiler_lowers_host_sub_assignments() {
    let stats = FieldId::new(3);
    let level = FieldId::new(4);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", HostTypeId::new(77));
    let stats_ty = register_registry_host_type(&mut registry, "Stats", HostTypeId::new(78));
    register_registry_host_field_with_type(
        &mut registry,
        player,
        "Player",
        "stats",
        stats,
        true,
        Some("Stats"),
    );
    register_registry_host_field(&mut registry, stats_ty, "Stats", "level", level, true);
    let code = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.stats.level -= 2;
    return player.stats.level;
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect("host sub assignment should compile");
    assert!(has_host_mutate_target(
        &code,
        vela_host::resolved::HostMutationOp::Sub,
        &[HostPathPart::Field(stats), HostPathPart::Field(level)],
        0
    ));
}
#[test]
fn compiler_lowers_host_numeric_compound_assignments() {
    let stats = FieldId::new(3);
    let level = FieldId::new(4);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", HostTypeId::new(77));
    let stats_ty = register_registry_host_type(&mut registry, "Stats", HostTypeId::new(78));
    register_registry_host_field_with_type(
        &mut registry,
        player,
        "Player",
        "stats",
        stats,
        true,
        Some("Stats"),
    );
    register_registry_host_field(&mut registry, stats_ty, "Stats", "level", level, true);
    let code = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.stats.level *= 3;
    player.stats.level /= 2;
    player.stats.level %= 5;
    return player.stats.level;
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect("host numeric compound assignments should compile");
    let target = [HostPathPart::Field(stats), HostPathPart::Field(level)];
    assert!(has_host_mutate_target(
        &code,
        vela_host::resolved::HostMutationOp::Mul,
        &target,
        0
    ));
    assert!(has_host_mutate_target(
        &code,
        vela_host::resolved::HostMutationOp::Div,
        &target,
        0
    ));
    assert!(has_host_mutate_target(
        &code,
        vela_host::resolved::HostMutationOp::Rem,
        &target,
        0
    ));
}
#[test]
fn compiler_lowers_host_path_push_calls() {
    let inventory = FieldId::new(3);
    let rewards = FieldId::new(4);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", HostTypeId::new(77));
    let inventory_ty = register_registry_host_type(&mut registry, "Inventory", HostTypeId::new(78));
    register_registry_host_field_with_type(
        &mut registry,
        player,
        "Player",
        "inventory",
        inventory,
        true,
        Some("Inventory"),
    );
    register_registry_host_field(
        &mut registry,
        inventory_ty,
        "Inventory",
        "rewards",
        rewards,
        true,
    );
    let code = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.inventory.rewards.push({
        let reward = "gold";
        reward
    });
    return 1;
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect("host path push should compile");
    assert!(has_host_mutate_target(
        &code,
        vela_host::resolved::HostMutationOp::Push,
        &[HostPathPart::Field(inventory), HostPathPart::Field(rewards)],
        0
    ));
}
#[test]
fn compiler_lowers_host_path_remove_calls() {
    let inventory = FieldId::new(3);
    let items = FieldId::new(4);
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = register_registry_host_type(&mut registry, "Player", HostTypeId::new(77));
    let inventory_ty = register_registry_host_type(&mut registry, "Inventory", HostTypeId::new(78));
    register_registry_host_field_with_type(
        &mut registry,
        player,
        "Player",
        "inventory",
        inventory,
        true,
        Some("Inventory"),
    );
    register_registry_host_field(
        &mut registry,
        inventory_ty,
        "Inventory",
        "items",
        items,
        true,
    );
    let code = compile_function_source_with_registry(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    let item_id = "gold";
    player.inventory.items[item_id].remove();
    return 1;
}
"#,
        "main",
        registry.compile_view(),
    )
    .expect("host path remove should compile");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| match &instruction.kind {
                UnlinkedInstructionKind::HostRemove {
                    target,
                    dynamic_args,
                    ..
                } =>
                    dynamic_args.len() == 1
                        && host_target_parts(&code, *target)
                            == [
                                HostPathPart::Field(inventory),
                                HostPathPart::Field(items),
                                HostPathPart::DynKey { arg: 0 },
                            ],
                _ => false,
            })
    );
}
