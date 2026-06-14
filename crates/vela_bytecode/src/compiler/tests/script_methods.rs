use super::*;

#[test]
fn compiler_registers_inherent_impl_methods_as_script_dispatch_targets() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Player { level: i64 }
impl Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}
fn main() {
    return Player { level: 7 }.bonus(5);
}
"#,
    )
    .expect("inherent impl method should compile as hidden dispatch target");
    let method = program
        .script_method("Player", "bonus")
        .expect("script inherent method dispatch target");
    assert_eq!(method.params, ["self", "amount"]);
    let method_id = stable_test_inherent_method_id("main::Player", "bonus");
    assert_eq!(program.script_method_id("Player", "bonus"), Some(method_id));
    assert_eq!(
        program
            .script_method_by_id("Player", method_id)
            .expect("script method by stable id")
            .params,
        ["self", "amount"]
    );
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == method_id
    )));
    assert!(program.function("bonus").is_none());
}

#[test]
fn compiler_lowers_self_record_compound_assignment_in_script_method() {
    let program = compile_program_source(
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
    .expect("self record compound assignment should compile");
    let method = program
        .script_method("Counter", "inc")
        .expect("script method dispatch target");
    assert!(method.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::SetRecordSlot { field, .. }
            | UnlinkedInstructionKind::SetRecordField { field, .. } if field == "counter"
    )));
}

#[test]
fn compiler_lowers_expression_receiver_record_compound_assignment() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Counter { counter: i64 }
fn make_counter() -> Counter {
    return Counter { counter: 1 };
}
fn main() {
    make_counter().counter += 1;
    return 0;
}
"#,
    )
    .expect("expression receiver record compound assignment should compile");
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::SetRecordSlot { field, .. }
            | UnlinkedInstructionKind::SetRecordField { field, .. } if field == "counter"
    )));
}

#[test]
fn compiler_keeps_static_script_receiver_on_method_id_path() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct Label { text: String }
impl Label {
    fn starts_with(self, prefix: String) -> bool {
        return self.text.starts_with(prefix);
    }
}
fn main() {
    let label = Label { text: "quest" };
    return label.starts_with("q");
}
"#,
    )
    .expect("static script receiver method should compile");
    let method_id = stable_test_inherent_method_id("main::Label", "starts_with");
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == method_id
    )));
    assert!(!main.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        UnlinkedInstructionKind::CallDynamicMethod { method, .. } if method == "starts_with"
    )));
}

#[test]
fn compiler_registers_impl_methods_as_script_dispatch_targets() {
    let program = compile_program_source(
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
    .expect("impl method should compile as hidden dispatch target");
    let method = program
        .script_method("Player", "bonus")
        .expect("script impl method dispatch target");
    assert_eq!(method.params, ["self", "amount"]);
    let method_id = stable_test_trait_method_id("main::BonusSource", "bonus");
    assert_eq!(program.script_method_id("Player", "bonus"), Some(method_id));
    assert_eq!(
        program
            .script_method_by_id("Player", method_id)
            .expect("script method by stable id")
            .params,
        ["self", "amount"]
    );
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == method_id
    )));
    assert!(program.function("bonus").is_none());
}

#[test]
fn compiler_registers_builtin_partial_eq_impl_without_source_trait_item() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
struct PlayerId { server: i64, id: i64 }
impl PartialEq for PlayerId {
    fn eq(self, other: PlayerId) -> bool {
        return self.server == other.server && self.id == other.id;
    }
}
fn main() {
    return PlayerId { server: 1, id: 7 } == PlayerId { server: 1, id: 7 };
}
"#,
    )
    .expect("builtin PartialEq impl should compile without declaring the trait");
    let method_id = stable_test_trait_method_id("PartialEq", "eq");
    assert_eq!(program.script_method_id("PlayerId", "eq"), Some(method_id));
    assert!(program.script_method_by_id("PlayerId", method_id).is_some());
}

#[test]
fn compiler_specializes_module_inherent_method_calls_by_method_id() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::model"),
            r#"
pub struct Player { level: i64 }
impl Player {
    fn bonus(self, amount) -> i64 {
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
    .expect("module inherent method should specialize by method id");
    let method_id = stable_test_inherent_method_id("game::model::Player", "bonus");
    let main = program
        .function("game::combat::main")
        .expect("game::combat::main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == method_id
    )));
}

#[test]
fn compiler_rejects_duplicate_receiver_script_methods() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self) -> i64; }
struct Player { level: i64 }
impl Player {
    fn bonus(self) -> i64 { return self.level; }
}
impl BonusSource for Player {
    fn bonus(self) -> i64 { return self.level; }
}
"#,
    )
    .expect_err("duplicate receiver methods should be rejected");

    assert_eq!(
        semantic_diagnostic_codes(error),
        ["hir::duplicate_script_method"]
    );
}

#[test]
fn compiler_lowers_named_and_default_script_method_args() {
    let program = compile_program_source(
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
    return player.bonus(offset = 4, amount = 5);
}
"#,
    )
    .expect("named/default method call should compile");
    let method_id = stable_test_trait_method_id("main::BonusSource", "bonus");
    let main = program.function("main").expect("main function");
    let args = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::CallMethodId {
                method_id: lowered,
                args,
                ..
            } if *lowered == method_id => Some(args),
            _ => None,
        })
        .expect("named/default method call should lower by stable id");
    assert_eq!(args.len(), 3);
    assert!(matches!(args[0], CallArgument::Register(_)));
    assert_eq!(args[1], CallArgument::Missing);
    assert!(matches!(args[2], CallArgument::Register(_)));
}
#[test]
fn compiler_registers_host_target_impl_methods_as_script_dispatch_targets() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return reflect::get(self, "level") + amount;
    }
}
fn main(player) {
    return player.bonus(5);
}
"#,
    )
    .expect("host target impl method should compile as hidden dispatch target");
    let method = program
        .script_method("Player", "bonus")
        .expect("host target script impl method dispatch target");
    assert_eq!(method.params, ["self", "amount"]);
    let method_id = stable_test_trait_method_id("main::BonusSource", "bonus");
    assert_eq!(program.script_method_id("Player", "bonus"), Some(method_id));
}
#[test]
fn compiler_registers_trait_default_methods_as_dispatch_targets() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self, amount) -> i64 { return self.level + amount; }
}
struct Player { level: i64 }
impl BonusSource for Player {}
fn main() {
    return Player { level: 7 }.bonus(5);
}
"#,
    )
    .expect("trait default method should compile as hidden dispatch target");
    let method = program
        .script_method("Player", "bonus")
        .expect("trait default method dispatch target");
    assert_eq!(method.params, ["self", "amount"]);
    let method_id = stable_test_trait_method_id("main::BonusSource", "bonus");
    assert_eq!(program.script_method_id("Player", "bonus"), Some(method_id));
    assert!(program.script_method_by_id("Player", method_id).is_some());
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == method_id
    )));
    assert!(program.function("bonus").is_none());
}
#[test]
fn compiler_specializes_self_method_calls_by_method_id() {
    let program = compile_program_source(
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
    .expect("self method calls should specialize by method id");
    let label_id = stable_test_trait_method_id("main::BonusSource", "label");
    let summary = program
        .script_method("Player", "summary")
        .expect("trait default summary method");
    assert!(summary.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == label_id
    )));
}
#[test]
fn compiler_specializes_captured_receiver_method_calls_by_method_id() {
    let program = compile_program_source(
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
    return bonus(null);
}
"#,
    )
    .expect("captured receiver method should specialize by method id");
    let method_id = stable_test_trait_method_id("main::BonusSource", "bonus");
    let main = program.function("main").expect("main function");
    let closure = main
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            UnlinkedInstructionKind::MakeClosure { function, .. } => {
                main.nested_function(*function)
            }
            _ => None,
        })
        .expect("capturing closure code");
    assert!(closure.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == method_id
    )));
}
#[test]
fn compiler_specializes_binding_pattern_receiver_method_calls_by_method_id() {
    let program = compile_program_source(
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
    .expect("binding pattern receiver method should specialize by method id");
    let method_id = stable_test_trait_method_id("main::BonusSource", "bonus");
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == method_id
    )));
}
#[test]
fn compiler_specializes_record_variant_field_method_calls_by_method_id() {
    let program = compile_program_source(
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
    .expect("record variant field receiver method should specialize by method id");
    let method_id = stable_test_trait_method_id("main::BonusSource", "bonus");
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == method_id
    )));
}
#[test]
fn compiler_specializes_tuple_variant_field_method_calls_by_method_id() {
    let program = compile_program_source(
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
    .expect("tuple variant field receiver method should specialize by method id");
    let method_id = stable_test_trait_method_id("main::BonusSource", "bonus");
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == method_id
    )));
}
#[test]
fn compiler_specializes_let_record_method_calls_by_method_id() {
    let program = compile_program_source(
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
    .expect("let-bound script record method should specialize by method id");
    let method_id = stable_test_trait_method_id("main::BonusSource", "bonus");
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == method_id
    )));
}
#[test]
fn compiler_specializes_typed_parameter_method_calls_by_method_id() {
    let program = compile_program_source(
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
    .expect("typed script parameter method should specialize by method id");
    let method_id = stable_test_trait_method_id("main::BonusSource", "bonus");
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == method_id
    )));
}
#[test]
fn compiler_specializes_typed_let_method_calls_by_method_id() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource { fn bonus(self, amount) -> i64; }
struct Player { level: i64 }
impl BonusSource for Player {
    fn bonus(self, amount) -> i64 {
        return self.level + amount;
    }
}
fn source() {
    return Player { level: 7 };
}
fn main() {
    let player: Player = source();
    return player.bonus(5);
}
"#,
    )
    .expect("typed let method should specialize by method id");
    let method_id = stable_test_trait_method_id("main::BonusSource", "bonus");
    let main = program.function("main").expect("main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == method_id
    )));
}
#[test]
fn compiler_specializes_module_typed_parameter_method_calls_by_method_id() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
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
            ModulePath::from_qualified("game::combat"),
            r#"
use game::model::Player
pub fn main(player: Player) {
    return player.bonus(5);
}
"#,
        ),
    ])
    .expect("module typed parameter method should specialize by method id");
    let method_id = stable_test_trait_method_id("game::model::BonusSource", "bonus");
    let main = program
        .function("game::combat::main")
        .expect("game::combat::main function");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallMethodId {
            method_id: lowered,
            ..
        } if lowered == method_id
    )));
}
#[test]
fn compiler_indexes_script_methods_by_receiver_and_method_id() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
trait BonusSource {
    fn bonus(self) -> i64 { return self.value; }
}
struct Player { value: i64 }
struct Monster { value: i64 }
impl BonusSource for Player {}
impl BonusSource for Monster {}
fn main() {
    return Player { value: 1 }.bonus() + Monster { value: 2 }.bonus();
}
"#,
    )
    .expect("shared trait method id should index per receiver");
    let method_id = stable_test_trait_method_id("main::BonusSource", "bonus");
    assert!(program.script_method_by_id("Player", method_id).is_some());
    assert!(program.script_method_by_id("Monster", method_id).is_some());
    assert!(program.script_method_by_id("Missing", method_id).is_none());
}
