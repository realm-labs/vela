use super::*;

#[test]
fn compiler_emits_script_calls_for_imported_aliases_across_modules() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::main"),
            r#"
use game::reward::grant as give_reward
fn main() {
    return give_reward(4);
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("game::reward"),
            r#"
pub fn grant(amount) {
    return amount + 1;
}
"#,
        ),
    ])
    .expect("cross-module imported script function should compile");
    let main = program
        .function("game::main::main")
        .expect("qualified main function");
    assert!(program.function("game::reward::grant").is_some());
    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::CallFunction { name, .. } if name == "game::reward::grant"
    )));
    assert!(!main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::CallNative { name, .. } if name == "give_reward"
    )));
}
#[test]
fn compiler_keeps_same_named_functions_in_separate_modules() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::main"),
            r#"
use game::reward::main as reward_main
fn main() {
    return reward_main();
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("game::reward"),
            r#"
pub fn main() {
    return 7;
}
"#,
        ),
    ])
    .expect("same-named cross-module functions should compile");
    assert!(program.function("game::main::main").is_some());
    assert!(program.function("game::reward::main").is_some());
    let main = program
        .function("game::main::main")
        .expect("qualified main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::CallFunction { name, .. } if name == "game::reward::main"
    )));
}
#[test]
fn compiler_uses_hir_type_symbols_for_imported_constructors() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::main"),
            r#"
use game::reward::Reward as Prize
use game::damage::Damage as Hit
fn make_reward() {
    return Prize { count: 2 };
}
fn make_damage() {
    return Hit::Physical { amount: 7 };
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("game::reward"),
            r#"
pub struct Reward { count: i64 }
"#,
        ),
        ModuleSource::new(
            SourceId::new(3),
            ModulePath::from_qualified("game::damage"),
            r#"
pub enum Damage { Physical { amount: i64 } }
"#,
        ),
    ])
    .expect("imported constructors should compile through HIR type symbols");
    let reward = program
        .function("game::main::make_reward")
        .expect("qualified reward function");
    let damage = program
        .function("game::main::make_damage")
        .expect("qualified damage function");
    assert!(reward.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::MakeRecord { type_name, .. } if type_name == "game::reward::Reward"
    )));
    assert!(damage.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::MakeEnum { enum_name, variant, .. }
            if enum_name == "game::damage::Damage" && variant == "Physical"
    )));
}

#[test]
fn compiler_lowers_imported_global_roots_to_qualified_host_globals() {
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
        .expect("Player::level should register");
    let program = compile_module_sources_with_registry(
        &[
            ModuleSource::new(
                SourceId::new(1),
                ModulePath::from_qualified("game::main"),
                r#"
use game::state::state
fn main() {
    state.level += 2;
    return state.level;
}
"#,
            ),
            ModuleSource::new(
                SourceId::new(2),
                ModulePath::from_qualified("game::state"),
                r#"
pub global state: Player;
"#,
            ),
        ],
        registry.compile_view(),
    )
    .expect("imported global root should compile");
    let main = program
        .function("game::main::main")
        .expect("qualified main function");
    let slot = program
        .global_slot("game::state::state")
        .expect("global slot should be assigned");
    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::LoadGlobal { global, slot: Some(actual), .. }
            if global == "game::state::state" && *actual == slot
    )));
}

#[test]
fn compiler_uses_hir_type_symbols_for_imported_match_patterns() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::main"),
            r#"
use game::damage::Damage as Hit
fn main() {
    let damage = Hit::Physical { amount: 7 };
    match damage {
        Hit::Physical { amount } => { return amount; },
        _ => { return 0; },
    }
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("game::damage"),
            r#"
pub enum Damage { Physical { amount: i64 } }
"#,
        ),
    ])
    .expect("imported match patterns should compile through HIR type symbols");
    let main = program
        .function("game::main::main")
        .expect("qualified main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::EnumTagEqual { enum_name, variant, .. }
            if enum_name == "game::damage::Damage" && variant == "Physical"
    )));
}
#[test]
fn compiler_uses_hir_facts_for_qualified_function_and_const_paths() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::main"),
            r#"
fn main() {
    return game::reward::grant() + game::config::BONUS;
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("game::reward"),
            r#"
pub fn grant() {
    return 4;
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(3),
            ModulePath::from_qualified("game::config"),
            r#"
pub const BONUS: i64 = 5;
"#,
        ),
    ])
    .expect("qualified function and const paths should compile");
    let main = program
        .function("game::main::main")
        .expect("qualified main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::CallFunction { name, .. } if name == "game::reward::grant"
    )));
    assert!(
        main.constants
            .contains(&Constant::Scalar(vela_common::ScalarValue::I64(5)))
    );
}
