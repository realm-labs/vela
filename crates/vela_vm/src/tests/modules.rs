use super::*;

#[test]
fn runs_compiled_cross_module_imported_script_call() {
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
    .expect("compile imported cross-module script call");

    assert_eq!(
        Vm::new().run_program(&program, "game::main::main", &[]),
        Ok(Value::Int(5))
    );
}

#[test]
fn runs_compiled_same_named_cross_module_functions() {
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
    .expect("compile same-named cross-module functions");

    assert_eq!(
        Vm::new().run_program(&program, "game::main::main", &[]),
        Ok(Value::Int(7))
    );
}

#[test]
fn runs_compiled_cross_module_imported_const_expression() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::main"),
            r#"
use game::tuning::BONUS as REWARD

fn main() {
    return REWARD + 1;
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("game::tuning"),
            r#"
use game::base::BASE as START

pub const BONUS: int = START + 1;
"#,
        ),
        ModuleSource::new(
            SourceId::new(3),
            ModulePath::from_qualified("game::base"),
            r#"
pub const BASE: int = 4;
"#,
        ),
    ])
    .expect("compile imported cross-module const expression");

    assert_eq!(
        Vm::new().run_program(&program, "game::main::main", &[]),
        Ok(Value::Int(6))
    );
}

#[test]
fn runs_compiled_cross_module_imported_type_constructors() {
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
pub struct Reward { count: int }
"#,
        ),
        ModuleSource::new(
            SourceId::new(3),
            ModulePath::from_qualified("game::damage"),
            r#"
pub enum Damage { Physical { amount: int } }
"#,
        ),
    ])
    .expect("compile imported cross-module type constructors");
    let mut reward_fields = BTreeMap::new();
    reward_fields.insert("count".into(), Value::Int(2));
    let mut damage_fields = BTreeMap::new();
    damage_fields.insert("amount".into(), Value::Int(7));

    assert_eq!(
        Vm::new().run_program(&program, "game::main::make_reward", &[]),
        Ok(Value::Record {
            type_name: "game::reward::Reward".into(),
            fields: ScriptFields::from_pairs("game::reward::Reward", reward_fields),
        })
    );
    assert_eq!(
        Vm::new().run_program(&program, "game::main::make_damage", &[]),
        Ok(Value::Enum {
            enum_name: "game::damage::Damage".into(),
            variant: "Physical".into(),
            fields: ScriptFields::from_pairs("game::damage::Damage::Physical", damage_fields),
        })
    );
}

#[test]
fn runs_cross_module_imported_constructor_defaults() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::main"),
            r#"
use game::reward::Reward as Prize

fn main() {
    let reward = Prize {};
    return reward.count + reward.item_id.len();
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("game::reward"),
            r#"
pub const BASE_COUNT = 5

pub struct Reward {
    item_id: string = "gold",
    count: int = BASE_COUNT + 2,
}
"#,
        ),
    ])
    .expect("compile imported constructor defaults");

    assert_eq!(
        Vm::new().run_program(&program, "game::main::main", &[]),
        Ok(Value::Int(11))
    );
}

#[test]
fn runs_compiled_cross_module_imported_match_patterns() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::main"),
            r#"
use game::damage::Damage as Hit

fn main() {
    let damage = Hit::Physical { amount: 7 };
    match damage {
        Hit::Magical { amount } => { return amount + 100; },
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
pub enum Damage {
    Physical { amount: int },
    Magical { amount: int },
}
"#,
        ),
    ])
    .expect("compile imported cross-module match pattern");

    assert_eq!(
        Vm::new().run_program(&program, "game::main::main", &[]),
        Ok(Value::Int(7))
    );
}

#[test]
fn runs_compiled_cross_module_qualified_function_and_const_paths() {
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
pub const BONUS: int = 5;
"#,
        ),
    ])
    .expect("compile qualified cross-module paths");

    assert_eq!(
        Vm::new().run_program(&program, "game::main::main", &[]),
        Ok(Value::Int(9))
    );
}
