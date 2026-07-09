use super::*;
use crate::verification::VerificationErrorKind;
use crate::{
    CacheSiteKind, CallArgument, Register, UnlinkedCodeObject, UnlinkedInstruction,
    UnlinkedInstructionKind, UnlinkedProgram,
};
use vela_def::{DefPath, FunctionId, MethodId};

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

mod call_diagnostics;
mod closures_and_bindings;
mod diagnostics;
mod expressions;
mod literals_and_calls;
mod loops_and_errors;
mod module_resolution;
mod script_methods;
mod type_contract_constructors;
mod value_method_shapes;
