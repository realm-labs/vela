use super::*;
use crate::verification::VerificationErrorKind;
use crate::{
    CacheSiteKind, CallArgument, Register, UnlinkedCodeObject, UnlinkedInstruction,
    UnlinkedInstructionKind, UnlinkedProgram,
};
use vela_def::{
    DefPath, FieldId, FunctionId, MethodId, script_inherent_method_id, script_trait_method_id,
};
use vela_host::target::HostPathPart;

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
    script_trait_method_id(trait_name, method_name)
}

fn stable_test_inherent_method_id(type_name: &str, method_name: &str) -> MethodId {
    script_inherent_method_id(type_name, method_name)
}

#[test]
fn compiler_entry_points_return_unlinked_bytecode() {
    let program: super::CompiledProgram = compile_program_source(
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
fn compiler_lowers_verified_mir_budget_points_to_instruction_metadata() {
    let program = compile_program_source(
        SourceId::new(3),
        "fn helper(value) { return value + 1; } fn main() { return helper(4); }",
    )
    .expect("budget schedule should compile");
    let main = program.function("main").expect("main function");
    let call = main
        .instructions
        .iter()
        .find(|instruction| {
            matches!(
                instruction.kind,
                UnlinkedInstructionKind::CallFunction { .. }
            )
        })
        .expect("MIR call point lowers to bytecode");

    assert_eq!(call.execution_units, 1);
    assert!(
        !program
            .function("helper")
            .expect("helper function")
            .instructions
            .iter()
            .any(|instruction| instruction.execution_units != 0)
    );
}

#[test]
fn linked_artifact_rejects_bytecode_that_drops_verified_mir_budget_points() {
    let mut program = compile_program_source(
        SourceId::new(4),
        "fn helper() { return 1; } fn main() { return helper(); }",
    )
    .expect("budget verification fixture compiles");
    program
        .bytecode
        .function_mut("main")
        .expect("main function")
        .instructions
        .iter_mut()
        .for_each(|instruction| instruction.execution_units = 0);

    assert!(matches!(
        crate::Linker::new().link_compiled_program(&program),
        Err(crate::linker::LinkError::MirBudgetScheduleMismatch {
            expected_units: 1,
            actual_units: 0,
            ..
        })
    ));
}

#[test]
fn contextual_compound_assignment_keeps_dynamic_call_results_generic() {
    let program = compile_program_source(
        SourceId::new(5),
        r#"
fn main() {
    let total = 0;
    let add = |value| value + 1;
    total += add(2);
    return total;
}
"#,
    )
    .expect("dynamic closure result must not become an unproved typed MIR operand");
    let main = program.function("main").expect("main function");

    assert!(
        main.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, UnlinkedInstructionKind::Add { .. }))
    );
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
    reward.gold = reward.gold + 1;
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
    assert!(site_kinds.contains(&CacheSiteKind::RecordFieldWrite));
    assert!(site_kinds.contains(&CacheSiteKind::HostPathRead));
    assert!(site_kinds.contains(&CacheSiteKind::HostPathWrite));
    let record_write_site = main
        .cache_sites
        .sites()
        .iter()
        .find(|site| site.kind == CacheSiteKind::RecordFieldWrite)
        .expect("record field write cache site should exist");
    assert!(matches!(
        main.instructions
            .get(record_write_site.instruction_offset.0)
            .map(|instruction| &instruction.kind),
        Some(UnlinkedInstructionKind::SetRecordSlot { .. })
    ));
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

#[test]
fn compiler_evaluates_schema_default_block_lets_from_hir_locals() {
    compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    count: i64 = { let base = 1; base + 1 },
}

fn main() {
    let reward = Reward {};
    return reward.count;
}
"#,
    )
    .expect("schema default block let should compile as a constant");
}

#[test]
fn compiler_uses_field_owned_bodies_for_enum_schema_defaults() {
    compile_program_source(
        SourceId::new(1),
        r#"
enum Reward {
    Record { count: i64 = { let base = 1; base + 1 } },
    Tuple(count: i64 = { let base = 2; base + 1 }),
}

fn main() {
    let record = Reward::Record {};
    let tuple = Reward::Tuple();
    return record.count + tuple.0;
}
"#,
    )
    .expect("record and tuple schema defaults should resolve by their HIR body IDs");
}

#[test]
fn compiler_resolves_host_field_after_dynamic_index_from_hir_receiver() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "IndexedPlayer",
            ))
            .host_runtime_id(11),
        )
        .expect("IndexedPlayer type should register");
    let _inventory = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "IndexedInventory",
            ))
            .host_runtime_id(12),
        )
        .expect("IndexedInventory type should register");
    let item = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "IndexedItem",
            ))
            .host_runtime_id(13),
        )
        .expect("IndexedItem type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "IndexedPlayer",
                    "inventory",
                ),
                player,
            )
            .type_hint(Some("IndexedInventory"))
            .host_runtime_id(20),
        )
        .expect("IndexedPlayer::inventory field should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "IndexedItem", "count"),
                item,
            )
            .type_hint(Some("i64"))
            .host_runtime_id(21),
        )
        .expect("IndexedItem::count field should register");
    let options = options::CompilerOptions::new().with_host_index_capability(
        "IndexedInventory",
        options::HostIndexCapabilityInfo {
            readable: true,
            key_type: Some("String".to_owned()),
            value_type: Some("IndexedItem".to_owned()),
            ..Default::default()
        },
    );

    let program = compile_program_source_with_options_and_registry(
        SourceId::new(1),
        r#"
fn item_count(player: IndexedPlayer, item_id: String) {
    return player.inventory[item_id].count;
}
"#,
        &options,
        registry.compile_view(),
    )
    .expect("host field after dynamic index should compile");
    let function = program
        .function("item_count")
        .expect("item_count should exist");
    assert!(
        function
            .cache_sites
            .sites()
            .iter()
            .any(|site| site.kind == CacheSiteKind::HostPathRead),
        "host field after dynamic index should lower to a host path read"
    );
    let target = function
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            UnlinkedInstructionKind::HostRead { target, .. } => function.host_target(target),
            _ => None,
        })
        .expect("host path read should carry a target plan");
    assert_eq!(
        target.parts.as_slice(),
        &[
            HostPathPart::Field(FieldId::new(20)),
            HostPathPart::DynKey { arg: 0 },
            HostPathPart::Field(FieldId::new(21)),
        ]
    );
}

#[test]
fn compiler_checks_host_index_key_type_from_hir_operand() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "KeyedPlayer",
            ))
            .host_runtime_id(41),
        )
        .expect("KeyedPlayer type should register");
    let _inventory = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "KeyedInventory",
            ))
            .host_runtime_id(42),
        )
        .expect("KeyedInventory type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field(
                    "host",
                    std::iter::empty::<&str>(),
                    "KeyedPlayer",
                    "inventory",
                ),
                player,
            )
            .type_hint(Some("KeyedInventory"))
            .host_runtime_id(44),
        )
        .expect("KeyedPlayer::inventory field should register");
    let options = options::CompilerOptions::new().with_host_index_capability(
        "KeyedInventory",
        options::HostIndexCapabilityInfo {
            readable: true,
            key_type: Some("i64".to_owned()),
            ..Default::default()
        },
    );

    let error = compile_program_source_with_options_and_registry(
        SourceId::new(1),
        r#"
fn item_count(player: KeyedPlayer) {
    return player.inventory["gold"];
}
"#,
        &options,
        registry.compile_view(),
    )
    .expect_err("wrong host index key type should fail");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::host_index_key_mismatch"]
    );
}

#[test]
fn compiler_checks_read_only_host_field_from_hir_receiver() {
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

    let error = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn bump(player: ReadOnlyPlayer) {
    player.level = 2;
    return 1;
}
"#,
        registry.compile_view(),
    )
    .expect_err("read-only host field assignment should fail");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["analysis::field_not_writable"]
    );
}

#[test]
fn compiler_resolves_param_default_field_receiver_from_hir() {
    let mut registry = vela_registry::DefinitionRegistry::new();
    let player = registry
        .register_type(
            vela_registry::TypeDef::new(DefPath::ty(
                "host",
                std::iter::empty::<&str>(),
                "DefaultPlayer",
            ))
            .host_runtime_id(31),
        )
        .expect("DefaultPlayer type should register");
    registry
        .register_field(
            vela_registry::FieldDef::new(
                DefPath::field("host", std::iter::empty::<&str>(), "DefaultPlayer", "level"),
                player,
            )
            .type_hint(Some("i64"))
            .host_runtime_id(32),
        )
        .expect("DefaultPlayer::level field should register");

    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
fn sample(player: DefaultPlayer, level = player.level) {
    return level;
}
"#,
        registry.compile_view(),
    )
    .expect("parameter-default host field should compile");
    let function = program.function("sample").expect("sample should exist");
    assert!(
        function
            .cache_sites
            .sites()
            .iter()
            .any(|site| site.kind == CacheSiteKind::HostPathRead),
        "parameter-default field access should lower through HostAccess"
    );
}

#[test]
fn compiler_resolves_method_call_receiver_from_hir() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Counter {
    value: i64,
}

impl Counter {
    fn add(self, amount: i64) {
        return self.value + amount;
    }
}

fn sample(counter: Counter) {
    return counter.add(2);
}
"#,
    )
    .expect("script method call should compile");
    let function = program.function("sample").expect("sample should exist");
    assert!(
        function.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::CallMethodId { .. }
        )),
        "script method call should lower to a resolved method call"
    );
}

#[test]
fn compiler_resolves_record_field_read_receiver_from_hir() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Counter {
    value: i64,
}

fn sample(counter: Counter) {
    return counter.value;
}
"#,
    )
    .expect("record field read should compile");
    let function = program.function("sample").expect("sample should exist");
    assert!(
        function.instructions.iter().any(|instruction| matches!(
            instruction.kind,
            UnlinkedInstructionKind::GetRecordSlot { .. }
        )),
        "typed record field read should lower to a resolved slot"
    );
}

mod block_tail_semantics;
mod call_diagnostics;
mod closures_and_bindings;
mod contract_delegation;
mod diagnostic_contracts;
mod diagnostics;
mod expressions;
mod literal_validation;
mod literals_and_calls;
mod loops_and_errors;
mod module_resolution;
mod phase0_frozen_contracts;
mod script_methods;
mod type_contract_constructors;
mod value_method_shapes;
