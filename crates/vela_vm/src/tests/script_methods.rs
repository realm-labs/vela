use super::*;
use crate::owned_value::OwnedValue;

fn run_script_method_program(
    vm: &Vm,
    program: &UnlinkedProgram,
    entry: &str,
    args: &[OwnedValue],
) -> VmResult<OwnedValue> {
    let mut budget = ExecutionBudget::unbounded();
    run_linked_test_program_with_budget(vm, program, entry, args, &mut budget)
}

#[test]
fn runs_compiled_script_value_methods() {
    let program = compile_standard_program_source(
            SourceId::new(1),
            r#"
fn main() {
    let values = [1, 2, 3];
    let rewards = {"gold": 4, "xp": 6};
    let empty = [];
    values.push(4);
    let popped = values.pop();
    let missing_pop = empty.pop();
    rewards.set("quest", 8);
    let missing_get = rewards.get("missing_before");
    let removed = rewards.remove("gold");
    let missing_remove = rewards.remove("missing_after");
    let keys = rewards.keys().collect_array();
    let amounts = rewards.values().collect_array();
    let entries = rewards.entries().collect_array();
    if empty.is_empty() && values.len() == 3 && option::unwrap_or(popped, 0) == 4 && rewards.len() == 2 && ("gold").len() == 4
        && ("gold").contains("ol") && ("quest").starts_with("que") && ("quest").ends_with("st")
        && option::is_none(missing_pop)
        && option::unwrap_or(removed, 0) == 4
        && option::is_none(missing_get) && option::is_none(missing_remove)
        && rewards.has("quest") && option::unwrap_or(rewards.get("xp"), 0) == 6 && rewards.get_or("missing", 10) == 10
        && keys[0] == "xp" && keys[1] == "quest"
        && amounts[0] == 6 && amounts[1] == 8
        && entries[0].key == "xp" && entries[1].value == 8 {
        return values.len();
    }
    return 0;
}
"#,
        )
        .expect("compile script value methods");

    let mut vm = Vm::new();
    vm.register_standard_natives();

    assert_eq!(
        run_script_method_program(&vm, &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(3)))
    );
}

#[test]
fn runs_script_method_self_record_compound_assignment() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
struct Counter { counter: i64 }
impl Counter {
    fn inc(self) {
        self.counter += 1;
    }
}
fn main() {
    let counter = Counter { counter: 1 };
    counter.inc();
    return counter.counter;
}
"#,
    )
    .expect("compile self record compound assignment");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(2)))
    );
}

#[test]
fn runs_compiled_script_impl_method_dispatch() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(5);
}
"#,
    )
    .expect("compile script impl method dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_inherent_script_impl_method_dispatch() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
struct Player { level: i64 }

impl Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(5);
}
"#,
    )
    .expect("compile inherent script impl method dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_script_method_named_and_default_args() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount, multiplier = 2, offset = 1) -> i64;
}
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount, multiplier = 2, offset = 1) -> i64 {
        return self.level + amount * multiplier + offset;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(offset = 4, amount = 5)
        + Player { level: 3 }.bonus(amount = 2);
}
"#,
    )
    .expect("compile script method named/default args");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(29)))
    );
}

#[test]
fn runs_compiled_typed_parameter_method_id_dispatch() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main(player: Player) {
    return player.bonus(5);
}
"#,
    )
    .expect("compile typed parameter method id dispatch");
    let player = OwnedValue::Record {
        type_name: "Player".to_owned(),
        fields: ScriptFields::from_pairs(
            "Player",
            [(
                "level".to_owned(),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(7)),
            )],
        ),
    };

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[player]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_immediate_script_method_id_dispatch() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    return Player { level: 7 }.bonus(5);
}
"#,
    )
    .expect("compile immediate script method id dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_trait_default_method_dispatch() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount) -> i64 { return self.level + amount; }
    fn label(self) -> String { return self.name; }
}
struct Player { level: i64, name: String }

impl BonusSource for Player {}

fn main() {
    let player = Player { level: 7, name: "hero" };
    if player.label() == "hero" {
        return player.bonus(5) + 4;
    }
    return 0;
}
"#,
    )
    .expect("compile trait default method dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(16)))
    );
}

#[test]
fn runs_compiled_self_method_id_dispatch() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn label(self) -> String;
    fn summary(self) -> String { return self.label(); }
}
struct Player { name: String }

impl BonusSource for Player {
    fn label(self) -> String {
        return self.name;
    }
}

fn main() {
    return Player { name: "hero" }.summary();
}
"#,
    )
    .expect("compile self method id dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::String("hero".to_owned()))
    );
}

#[test]
fn runs_compiled_captured_receiver_method_id_dispatch() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    let bonus = |ignored| player.bonus(5);
    return bonus(true);
}
"#,
    )
    .expect("compile captured receiver method id dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_binding_pattern_receiver_method_id_dispatch() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    return match player {
        bound => bound.bonus(5),
    };
}
"#,
    )
    .expect("compile binding pattern receiver method id dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_record_variant_field_method_id_dispatch() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

enum Event {
    Grant { player: Player },
    None,
}

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    let event = Event::Grant { player: Player { level: 7 } };
    return match event {
        Event::Grant { player } => player.bonus(5),
        _ => 0,
    };
}
"#,
    )
    .expect("compile record variant field method id dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn runs_compiled_tuple_variant_field_method_id_dispatch() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

enum Event {
    Grant(player: Player),
    None,
}

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

fn main() {
    let event = Event::Grant(Player { level: 7 });
    return match event {
        Event::Grant(player) => player.bonus(5),
        _ => 0,
    };
}
"#,
    )
    .expect("compile tuple variant field method id dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}

#[test]
fn explicit_impl_method_overrides_trait_default_dispatch() {
    let program = compile_test_program(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount) -> i64 { return self.level + amount; }
}
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return amount * 2;
    }
}

fn main() {
    let player = Player { level: 7 };
    return player.bonus(5);
}
"#,
    )
    .expect("compile explicit impl method override");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(10)))
    );
}

#[test]
fn runs_compiled_module_qualified_script_impl_method_dispatch() {
    let program = compile_test_modules(&[ModuleSource::new(
        SourceId::new(1),
        vela_package::PackageId::anonymous(),
        ModulePath::from_qualified("game::combat"),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}

pub fn main() {
    let player = Player { level: 10 };
    return player.bonus(4);
}
"#,
    )])
    .expect("compile module-qualified script impl method dispatch");

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "game::combat::main", &[]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(14)))
    );
}

#[test]
fn runs_compiled_module_typed_parameter_method_id_dispatch() {
    let program = compile_test_modules(&[
        ModuleSource::new(
            SourceId::new(1),
            vela_package::PackageId::anonymous(),
            ModulePath::from_qualified("game::model"),
            r#"
pub trait BonusSource { fn bonus(self, amount) -> i64; }
pub struct Player { level: i64 }

impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            vela_package::PackageId::anonymous(),
            ModulePath::from_qualified("game::combat"),
            r#"
use game::model::Player

pub fn main(player: Player) {
    return player.bonus(5);
}
"#,
        ),
    ])
    .expect("compile module typed parameter method id dispatch");
    let player = OwnedValue::Record {
        type_name: "game::model::Player".to_owned(),
        fields: ScriptFields::from_pairs(
            "game::model::Player",
            [(
                "level".to_owned(),
                OwnedValue::Scalar(vela_common::ScalarValue::I64(7)),
            )],
        ),
    };

    assert_eq!(
        run_script_method_program(&Vm::new(), &program, "game::combat::main", &[player]),
        Ok(OwnedValue::Scalar(vela_common::ScalarValue::I64(12)))
    );
}
