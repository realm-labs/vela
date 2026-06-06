use super::*;

#[test]
fn runtime_stages_source_file_defaulted_schema_addition_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
struct Reward {
    item_id: string
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
struct Reward {
    item_id: string
    count: int = 1
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged schema addition report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main"]);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(2))
    );
}

#[test]
fn runtime_stages_source_file_stable_id_schema_renames_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
struct Reward {
    #[id(101)]
    item_id: string
    #[id(102)]
    count: int
}

enum QuestProgress {
    #[id(201)]
    Active
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
struct Reward {
    #[id(101)]
    item: string
    #[id(102)]
    quantity: int
}

enum QuestProgress {
    #[id(201)]
    Started
    #[id(202)]
    Finished
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged stable-id schema rename report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main"]);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(2))
    );
}

#[test]
fn runtime_stages_source_file_required_schema_field_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
struct Reward {
    item_id: string
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
struct Reward {
    item_id: string
    count: int
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged schema field rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(report.errors[0].target.as_deref(), Some("Reward"));
    assert_changed_schema_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_removed_schema_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
struct Reward {
    item_id: string
    count: int
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged removed schema rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.removed");
    assert_eq!(report.errors[0].target.as_deref(), Some("Reward"));
    assert_removed_schema_repair_hint(&report);
    let HotReloadErrorKind::RemovedSchema {
        type_name,
        old_hash,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected removed schema rejection");
    };
    assert_eq!(type_name, "Reward");
    assert_ne!(*old_hash, 0);
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_schema_field_type_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
struct Reward {
    item_id: string
    count: int
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
struct Reward {
    item_id: string
    count: float
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged schema field type rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(report.errors[0].target.as_deref(), Some("Reward"));
    assert_changed_schema_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_defaulted_enum_variant_field_addition_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
enum QuestProgress {
    Active {
        quest_id: string
    }
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
enum QuestProgress {
    Active {
        quest_id: string
        count: int = 0
    }
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged enum variant field addition report");

    assert!(report.accepted);
    assert_eq!(report.changed_functions, vec!["main"]);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(2))
    );
}

#[test]
fn runtime_stages_source_file_required_enum_variant_field_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
enum QuestProgress {
    Active {
        quest_id: string
    }
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
enum QuestProgress {
    Active {
        quest_id: string
        count: int
    }
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged enum variant field rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(report.errors[0].target.as_deref(), Some("QuestProgress"));
    assert_changed_schema_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_enum_variant_field_type_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
enum QuestProgress {
    Active {
        quest_id: string
        count: int
    }
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
enum QuestProgress {
    Active {
        quest_id: string
        count: float
    }
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged enum variant field type rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(report.errors[0].target.as_deref(), Some("QuestProgress"));
    assert_changed_schema_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_removed_trait_impl_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
trait Damageable {
    fn damage(self) -> int { return self.level; }
}

struct Player {
    level: int
}

impl Damageable for Player {}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
trait Damageable {
    fn damage(self) -> int { return self.level; }
}

struct Player {
    level: int
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged removed trait impl rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.schema.abi_changed");
    assert_eq!(report.errors[0].target.as_deref(), Some("Player"));
    assert_changed_schema_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_added_trait_impl_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
trait Damageable {
    fn damage(self) -> int { return self.level; }
}

struct Player {
    level: int
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
trait Damageable {
    fn damage(self) -> int { return self.level; }
}

struct Player {
    level: int
}

impl Damageable for Player {}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged added trait impl report");

    assert!(report.accepted);
    assert!(report.changed_functions.contains(&"main".to_owned()));
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(2))
    );
}

#[test]
fn runtime_stages_source_file_removed_trait_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
trait Damageable {
    fn damage(self, amount: int) -> int;
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged removed trait rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.trait.removed_abi");
    assert_eq!(report.errors[0].target.as_deref(), Some("Damageable"));
    assert_removed_trait_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_trait_method_return_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
trait Damageable {
    fn damage(self, amount: int) -> int;
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
trait Damageable {
    fn damage(self, amount: int) -> float;
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged trait method return rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.trait.changed_abi");
    assert_eq!(report.errors[0].target.as_deref(), Some("Damageable"));
    assert_changed_trait_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_required_trait_method_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
trait Damageable {
    fn damage(self, amount: int) -> int;
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
trait Damageable {
    fn damage(self, amount: int) -> int;
    fn heal(self, amount: int) -> int;
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged required trait method rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.trait.changed_abi");
    assert_eq!(report.errors[0].target.as_deref(), Some("Damageable"));
    assert_changed_trait_abi_repair_hint(&report);
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_defaulted_trait_method_addition_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
trait Damageable {
    fn damage(self, amount: int) -> int;
}

fn main() {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
trait Damageable {
    fn damage(self, amount: int) -> int;
    fn heal(self, amount: int) -> int { return amount; }
}

fn main() {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged defaulted trait method addition report");

    assert!(report.accepted);
    assert_eq!(report.errors, Vec::new());
    assert!(report.changed_functions.contains(&"main".to_owned()));
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(2))
    );
}

#[test]
fn runtime_stages_source_file_event_parameter_reorder_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
#[event("monster.kill")]
fn on_kill(player_id: int, monster_id: int) {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
#[event("monster.kill")]
fn on_kill(monster_id: int, player_id: int) {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw(
            "on_kill",
            &[OwnedValue::Int(7), OwnedValue::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged event ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.changed_parameters");
    let HotReloadErrorKind::ChangedFunctionParameters { function, old, new } =
        &report.errors[0].error.kind
    else {
        panic!("expected changed function parameters");
    };
    assert_eq!(function, "on_kill");
    assert_eq!(old, &vec!["player_id".to_owned(), "monster_id".to_owned()]);
    assert_eq!(new, &vec!["monster_id".to_owned(), "player_id".to_owned()]);
    assert_eq!(
        runtime.call_raw(
            "on_kill",
            &[OwnedValue::Int(7), OwnedValue::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_event_target_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
#[event("monster.kill")]
fn on_kill(player_id: int, monster_id: int) {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
#[event("quest.complete")]
fn on_kill(player_id: int, monster_id: int) {
    return 2;
}
"#,
    );
    assert_eq!(
        runtime.call_raw(
            "on_kill",
            &[OwnedValue::Int(7), OwnedValue::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged event target rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.event_changed");
    let HotReloadErrorKind::ChangedFunctionEvent {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function event");
    };
    assert_eq!(function, "on_kill");
    assert_eq!(old.as_deref(), Some("monster.kill"));
    assert_eq!(new.as_deref(), Some("quest.complete"));
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call_raw(
            "on_kill",
            &[OwnedValue::Int(7), OwnedValue::Int(11)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_return_abi_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
fn main() -> int {
    return 1;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
fn main() -> float {
    return 2.0;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged return ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.return_abi_changed");
    assert_function_return_repair_hint(&report);
    assert_rendered_repair_hint(
        &report,
        "preserve the previous return type hint or restart with an explicit migration",
    );
    let HotReloadErrorKind::ChangedFunctionReturnAbi {
        function,
        old,
        new,
        source_span,
    } = &report.errors[0].error.kind
    else {
        panic!("expected changed function return ABI");
    };
    assert_eq!(function, "main");
    assert_eq!(old.as_deref(), Some("int"));
    assert_eq!(new.as_deref(), Some("float"));
    assert!(source_span.is_some());
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(1))
    );
}

#[test]
fn runtime_stages_source_file_required_parameter_addition_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
fn main(player_id: int) {
    return player_id;
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    stage_source_update(
        &mut runtime,
        r#"
fn main(player_id: int, amount: int) {
    return amount;
}
"#,
    );
    assert_eq!(
        runtime.call_raw(
            "main",
            &[OwnedValue::Int(7)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(7))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged required parameter rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(
        report.errors[0].code,
        "reload.function.required_added_parameters"
    );
    assert_required_parameter_repair_hint(&report);
    let HotReloadErrorKind::AddedFunctionParametersWithoutDefaults { function, added } =
        &report.errors[0].error.kind
    else {
        panic!("expected added required parameters");
    };
    assert_eq!(function, "main");
    assert_eq!(added, &vec!["amount".to_owned()]);
    assert_eq!(
        runtime.call_raw(
            "main",
            &[OwnedValue::Int(7)],
            CallOptions::unbounded(),
            &mut adapter,
            &mut tx
        ),
        Ok(OwnedValue::Int(7))
    );
}

#[test]
fn runtime_stages_source_file_script_function_access_rejection_until_safe_point() {
    let engine = Engine::builder()
        .execution_profile(ExecutionProfile::trusted())
        .build()
        .expect("engine should build");
    let mut runtime = runtime_from_hot_reload_source(
        engine,
        r#"
pub fn grant() {
    return 2;
}

fn main() {
    return grant();
}
"#,
    );
    let mut adapter = MockStateAdapter::new();
    let mut tx = HostAccess::new();

    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(2))
    );

    stage_source_update(
        &mut runtime,
        r#"
fn grant() {
    return 6;
}

fn main() {
    return 3;
}
"#,
    );
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(2))
    );

    let report = runtime
        .check_reload()
        .expect("check reload at safe point")
        .expect("staged script function access ABI rejection report");

    assert!(!report.accepted);
    assert_eq!(report.to_version, None);
    assert_eq!(report.errors[0].code, "reload.function.access_changed");
    assert_changed_function_access_rejection(&report, "grant");
    assert_eq!(
        runtime.call_raw("main", &[], CallOptions::unbounded(), &mut adapter, &mut tx),
        Ok(OwnedValue::Int(2))
    );
}
