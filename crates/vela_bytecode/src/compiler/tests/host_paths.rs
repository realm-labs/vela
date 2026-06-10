use super::*;
use crate::HostTargetPlanId;
use vela_common::HostTypeId;
use vela_def::{DefPath, TypeId};
use vela_host::target::HostPathPart;

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
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), owner_name, name),
                owner,
            )
            .host_runtime_id(id.get())
            .writable(writable),
        )
        .expect("test host field should register");
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
                vela_registry::FunctionSignature::new(params, None),
            )
            .host_runtime_id(runtime_id.get()),
        )
        .expect("test host method should register");
}

fn host_target_parts(code: &CodeObject, target: HostTargetPlanId) -> &[HostPathPart] {
    code.host_target(target)
        .expect("host target should exist")
        .parts
        .as_slice()
}

fn has_host_call(code: &CodeObject, method: HostMethodId, arg_count: usize) -> bool {
    code.instructions.iter().any(|instruction| {
        matches!(
            &instruction.kind,
            InstructionKind::HostCall {
                method: lowered_method,
                args,
                ..
            } if *lowered_method == method && args.len() == arg_count
        )
    })
}

fn has_host_call_target(
    code: &CodeObject,
    method: HostMethodId,
    expected: &[HostPathPart],
    dynamic_arg_count: usize,
) -> bool {
    code.instructions
        .iter()
        .any(|instruction| match &instruction.kind {
            InstructionKind::HostCall {
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
    code: &CodeObject,
    op: vela_host::resolved::HostMutationOp,
    expected: &[HostPathPart],
    dynamic_arg_count: usize,
) -> bool {
    code.instructions
        .iter()
        .any(|instruction| match &instruction.kind {
            InstructionKind::HostMutate {
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
    code: &CodeObject,
    expected: &[HostPathPart],
    dynamic_arg_count: usize,
) -> bool {
    code.instructions
        .iter()
        .any(|instruction| match &instruction.kind {
            InstructionKind::HostRead {
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
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    return player.level;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_type_id("Player", player_type)
            .with_host_field("level", level)
            .with_host_field_for_type("Player", "level", level, true),
    )
    .expect("typed host field read should compile");

    let Some(target) = code
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            InstructionKind::HostRead { target, .. } => Some(target),
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
        &CompilerOptions::new().with_host_type("Player"),
        registry.compile_view(),
    )
    .expect("registry host field read should compile");

    let Some(target) = code
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            InstructionKind::HostRead { target, .. } => Some(target),
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
        &CompilerOptions::new().with_host_type("Player"),
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
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.grant_exp(20);
    return 1;
}
"#,
        "main",
        &CompilerOptions::new().with_host_method("grant_exp", method),
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
            vela_registry::ParamDef::new("amount", Some("int")),
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
        &CompilerOptions::new().with_host_type("Player"),
        registry.compile_view(),
    )
    .expect("registry host method call should compile");

    assert!(has_host_call(&code, method, 1));
}

#[test]
fn compiler_lowers_named_and_default_host_method_args_from_compiler_options() {
    let method = HostMethodId::new(5);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(ctx) {
    ctx.emit(event = "player.level_checked");
    return 1;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_method("emit", method)
            .with_host_method_params(method, [("event", false), ("payload", true)]),
    )
    .expect("named/default host method args should compile");

    assert!(has_host_call(&code, method, 1));
}

#[test]
fn compiler_keeps_positional_host_method_args_variadic_with_metadata() {
    let method = HostMethodId::new(5);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(ctx) {
    ctx.emit("player.level_checked", 10, 42);
    return 1;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_method("emit", method)
            .with_host_method_params(method, [("event", false), ("payload", true)]),
    )
    .expect("positional host method args should stay variadic");

    assert!(has_host_call(&code, method, 3));
}

#[test]
fn compiler_reports_named_host_method_arg_diagnostics_from_compiler_options() {
    let method = HostMethodId::new(5);
    let error = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(ctx) {
    ctx.emit(evnt = "player.level_checked");
    return 1;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_method("emit", method)
            .with_host_method_params(method, [("event", false), ("payload", true)]),
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
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(ctx) {
    ctx.emit("player.level_checked");
    return 1;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_native_module_root("ctx")
            .with_host_method("emit", method),
    )
    .expect("local host method should shadow native module root");
    assert!(has_host_call(&code, method, 1));
}

#[test]
fn compiler_lowers_configured_host_method_calls_on_field_paths() {
    let inventory = FieldId::new(3);
    let method = HostMethodId::new(5);
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.inventory.add("gold", 20);
    return 1;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_method("add", method),
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
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player, item_id) {
    player.inventory.items[item_id].grant(20);
    return 1;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_field("items", items)
            .with_host_method("grant", method),
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
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.stats.level += 2;
    return player.stats.level;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("stats", stats)
            .with_host_field("level", level),
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
    let error = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player: Player) {
    player.id = 8;
    return player.id;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("id", id)
            .with_host_field_for_type("Player", "id", id, false),
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
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player, item_id) {
    player.inventory.items[item_id].count += 1;
    return player.inventory.items[item_id].count;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_field("items", items)
            .with_host_field("count", count),
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
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.quest_progress.count += 1;
    return player.quest_progress.count;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("quest_progress", quest_progress)
            .with_host_variant_field("count", count),
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
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.stats.level -= 2;
    return player.stats.level;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("stats", stats)
            .with_host_field("level", level),
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
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.stats.level *= 3;
    player.stats.level /= 2;
    player.stats.level %= 5;
    return player.stats.level;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("stats", stats)
            .with_host_field("level", level),
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
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    player.inventory.rewards.push("gold");
    return 1;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_field("rewards", rewards),
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
    let code = compile_function_source_with_options(
        SourceId::new(1),
        r#"
fn main(player) {
    let item_id = "gold";
    player.inventory.items[item_id].remove();
    return 1;
}
"#,
        "main",
        &CompilerOptions::new()
            .with_host_field("inventory", inventory)
            .with_host_field("items", items),
    )
    .expect("host path remove should compile");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| match &instruction.kind {
                InstructionKind::HostRemove {
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
