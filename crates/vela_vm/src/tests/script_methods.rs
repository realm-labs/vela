use super::*;
use crate::owned_value::OwnedValue;

#[test]
fn runs_compiled_script_value_methods() {
    let program = compile_program_source(
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
    let keys = rewards.keys();
    let amounts = rewards.values();
    let entries = rewards.entries();
    if empty.is_empty() && values.len() == 3 && option::unwrap_or(popped, 0) == 4 && rewards.len() == 2 && ("gold").len() == 4
        && ("gold").contains("ol") && ("quest").starts_with("que") && ("quest").ends_with("st")
        && option::is_none(missing_pop)
        && option::unwrap_or(removed, 0) == 4
        && option::is_none(missing_get) && option::is_none(missing_remove)
        && rewards.has("quest") && option::unwrap_or(rewards.get("xp"), 0) == 6 && rewards.get_or("missing", 10) == 10
        && keys[0] == "quest" && keys[1] == "xp"
        && amounts[0] == 8 && amounts[1] == 6
        && entries[0].key == "quest" && entries[1].value == 6 {
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
        vm.run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(3))
    );
}

#[test]
fn runs_compiled_script_impl_method_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
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
        Vm::new().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(12))
    );
}

#[test]
fn runs_compiled_script_method_named_and_default_args() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount, multiplier = 2, offset = 1) -> int;
}
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount, multiplier = 2, offset = 1) -> int {
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
        Vm::new().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(29))
    );
}

#[test]
fn runs_compiled_typed_parameter_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
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
        fields: ScriptFields::from_pairs("Player", [("level".to_owned(), OwnedValue::Int(7))]),
    };

    assert_eq!(
        Vm::new().run_program(&program, "main", &[player]),
        Ok(OwnedValue::Int(12))
    );
}

#[test]
fn runs_compiled_immediate_script_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
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
        Vm::new().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(12))
    );
}

#[test]
fn runs_compiled_trait_default_method_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount) -> int { return self.level + amount; }
    fn label(self) -> string { return self.name; }
}
struct Player { level: int, name: string }

impl BonusSource for Player {}

fn main() {
    let player = Player { level: 7, name: "hero" };
    return player.bonus(5) + player.label().len();
}
"#,
    )
    .expect("compile trait default method dispatch");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(16))
    );
}

#[test]
fn runs_compiled_self_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn label(self) -> string;
    fn summary(self) -> string { return self.label(); }
}
struct Player { name: string }

impl BonusSource for Player {
    fn label(self) -> string {
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
        Vm::new().run_program(&program, "main", &[]),
        Ok(OwnedValue::String("hero".to_owned()))
    );
}

#[test]
fn runs_compiled_captured_receiver_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}

fn main() {
    let player = Player { level: 7 };
    let bonus = |ignored| player.bonus(5);
    return bonus(null);
}
"#,
    )
    .expect("compile captured receiver method id dispatch");

    assert_eq!(
        Vm::new().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(12))
    );
}

#[test]
fn runs_compiled_binding_pattern_receiver_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
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
        Vm::new().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(12))
    );
}

#[test]
fn runs_compiled_host_ref_script_impl_method_dispatch() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return reflect::get(self, "level") + amount;
    }
}

fn main(player) {
    return player.bonus(5);
}
"#,
    )
    .expect("compile host ref script impl method dispatch");
    let mut adapter = host_adapter(host_ref, HostValue::Int(7));
    let mut tx = HostAccess::new();
    let mut vm = Vm::new();
    vm.register_reflection_natives(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        vm.run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Int(12))
    );
}

#[test]
fn host_ref_script_impl_dispatch_uses_registered_type_registry() {
    let host_ref = player_ref(3);
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return amount + 7;
    }
}

fn main(player) {
    return player.bonus(5);
}
"#,
    )
    .expect("compile host ref script impl method dispatch");
    let mut adapter = host_adapter(host_ref, HostValue::Int(7));
    let mut tx = HostAccess::new();
    let vm = Vm::new().with_type_registry(Arc::new(reflection_registry()));
    let mut host = HostExecution {
        adapter: &mut adapter,
        access: &mut tx,
        script_globals: None,
    };

    assert_eq!(
        vm.run_program_with_host(
            &program,
            "main",
            &[OwnedValue::HostRef(host_ref)],
            &mut host
        ),
        Ok(OwnedValue::Int(12))
    );
}

#[test]
fn runs_compiled_record_variant_field_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

enum Event {
    Grant { player: Player },
    None,
}

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
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
        Vm::new().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(12))
    );
}

#[test]
fn runs_compiled_tuple_variant_field_method_id_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

enum Event {
    Grant(player: Player),
    None,
}

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
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
        Vm::new().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(12))
    );
}

#[test]
fn explicit_impl_method_overrides_trait_default_dispatch() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount) -> int { return self.level + amount; }
}
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
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
        Vm::new().run_program(&program, "main", &[]),
        Ok(OwnedValue::Int(10))
    );
}

#[test]
fn runs_compiled_module_qualified_script_impl_method_dispatch() {
    let program = compile_module_sources(&[ModuleSource::new(
        SourceId::new(1),
        ModulePath::from_qualified("game::combat"),
        r#"
trait BonusSource { fn bonus(self, amount) -> int; }
struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
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
        Vm::new().run_program(&program, "game::combat::main", &[]),
        Ok(OwnedValue::Int(14))
    );
}

#[test]
fn runs_compiled_module_typed_parameter_method_id_dispatch() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::model"),
            r#"
pub trait BonusSource { fn bonus(self, amount) -> int; }
pub struct Player { level: int }

impl BonusSource for Player {
    fn bonus(self, amount) -> int {
        return self.level + amount;
    }
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
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
            [("level".to_owned(), OwnedValue::Int(7))],
        ),
    };

    assert_eq!(
        Vm::new().run_program(&program, "game::combat::main", &[player]),
        Ok(OwnedValue::Int(12))
    );
}
