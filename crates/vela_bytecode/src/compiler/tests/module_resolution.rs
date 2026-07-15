use super::*;
use vela_package::ModulePath;

#[test]
fn package_qualified_functions_and_types_survive_duplicate_source_paths() {
    let first = vela_package::PackageId::new("com.example.first").expect("first package");
    let second = vela_package::PackageId::new("com.example.second").expect("second package");
    let program = compile_test_modules(&[
        ModuleSource::new(
            SourceId::new(1),
            first.clone(),
            ModulePath::from_qualified("shared"),
            "pub struct State { value: i64 } pub fn run() { return State { value: 1 }; }",
        ),
        ModuleSource::new(
            SourceId::new(2),
            second.clone(),
            ModulePath::from_qualified("shared"),
            "pub struct State { value: i64 } pub fn run() { return State { value: 2 }; }",
        ),
    ])
    .expect("duplicate source paths in distinct packages should compile");
    let first_function = vela_def::script_function_id(first.as_str(), "shared::run");
    let second_function = vela_def::script_function_id(second.as_str(), "shared::run");
    let first_type = vela_def::script_type_id(first.as_str(), "shared::State", None);
    let second_type = vela_def::script_type_id(second.as_str(), "shared::State", None);

    assert!(program.function_by_id(first_function).is_some());
    assert!(program.function_by_id(second_function).is_some());
    assert!(program.function("shared::run").is_none());
    let linked = crate::Linker::new()
        .link_compiled_program(program)
        .expect("package-qualified program should link");
    assert!(linked.program().entry_point_by_id(first_function).is_some());
    assert!(
        linked
            .program()
            .entry_point_by_id(second_function)
            .is_some()
    );
    let linked_types = linked
        .program()
        .types()
        .map(|(_, ty)| ty.id)
        .collect::<BTreeSet<_>>();
    assert!(linked_types.contains(&first_type));
    assert!(linked_types.contains(&second_type));
}

#[test]
fn compiler_emits_script_calls_for_imported_aliases_across_modules() {
    let program = compile_test_modules(&[
        ModuleSource::new(
            SourceId::new(1),
            vela_package::PackageId::anonymous(),
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
            vela_package::PackageId::anonymous(),
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
    let program = compile_test_modules(&[
        ModuleSource::new(
            SourceId::new(1),
            vela_package::PackageId::anonymous(),
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
            vela_package::PackageId::anonymous(),
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
    let program = compile_test_modules(&[
        ModuleSource::new(
            SourceId::new(1),
            vela_package::PackageId::anonymous(),
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
            vela_package::PackageId::anonymous(),
            ModulePath::from_qualified("game::reward"),
            r#"
pub struct Reward { count: i64 }
"#,
        ),
        ModuleSource::new(
            SourceId::new(3),
            vela_package::PackageId::anonymous(),
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
    let program = compile_test_modules_with_registry(
        &[
            ModuleSource::new(
                SourceId::new(1),
                vela_package::PackageId::anonymous(),
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
                vela_package::PackageId::anonymous(),
                ModulePath::from_qualified("game::state"),
                r#"
pub extern state state: Player;
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
        UnlinkedInstructionKind::LoadExternState { state, slot: Some(actual), .. }
            if state == "game::state::state" && *actual == slot
    )));
}

#[test]
fn compiler_uses_hir_type_symbols_for_imported_match_patterns() {
    let program = compile_test_modules(&[
        ModuleSource::new(
            SourceId::new(1),
            vela_package::PackageId::anonymous(),
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
            vela_package::PackageId::anonymous(),
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
    let program = compile_test_modules(&[
        ModuleSource::new(
            SourceId::new(1),
            vela_package::PackageId::anonymous(),
            ModulePath::from_qualified("game::main"),
            r#"
fn main() {
    return game::reward::grant() + game::config::BONUS;
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            vela_package::PackageId::anonymous(),
            ModulePath::from_qualified("game::reward"),
            r#"
pub fn grant() {
    return 4;
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(3),
            vela_package::PackageId::anonymous(),
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
