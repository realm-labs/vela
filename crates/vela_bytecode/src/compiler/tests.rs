use super::*;
use crate::verification::VerificationErrorKind;
use crate::{
    CacheSiteKind, CallArgument, CodeObject, Instruction, InstructionKind, Program, Register,
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
fn compiler_boundary_rejects_invalid_program_bytecode() {
    let mut code = CodeObject::new("main", 1);
    code.push_instruction(Instruction::new(InstructionKind::Return {
        src: Register(2),
    }));
    let mut program = Program::new();
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
    let mut code = CodeObject::new("main", 1);
    code.push_instruction(Instruction::new(InstructionKind::Return {
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
                    [vela_registry::ParamDef::new("amount", Some("int"))],
                    None,
                ),
            )
            .with_id(FunctionId::new(7)),
        )
        .expect("give_reward function should register");
    let program = compile_program_source_with_registry(
        SourceId::new(1),
        r#"
global bonus: Int;

struct Reward {
    gold: Int,
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
            InstructionKind::LoadGlobal { cache_site, .. } => *cache_site,
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
    for (index, site) in main.cache_sites.sites().iter().enumerate() {
        assert_eq!(site.id.index(), index);
        assert_eq!(site.function, "main");
        assert!(main.instructions.get(site.instruction_offset.0).is_some());
    }
}

mod closures_and_bindings;
mod diagnostics;
mod expressions;
mod host_paths;
mod literals_and_calls;
mod loops_and_errors;
mod module_resolution;
mod script_methods;
