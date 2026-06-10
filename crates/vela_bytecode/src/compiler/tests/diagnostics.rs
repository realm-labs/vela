use super::*;

#[test]
fn compiler_rejects_duplicate_declarations_from_hir() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
fn main() { return 1; }
fn main() { return 2; }
"#,
    )
    .expect_err("duplicate function should fail before bytecode generation");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_declaration"))
    );
}
#[test]
fn compiler_rejects_duplicate_parameters_from_hir() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
fn main(amount, amount) {
    return amount;
}
"#,
    )
    .expect_err("duplicate parameter should fail before bytecode generation");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_parameter"))
    );
}
#[test]
fn compiler_rejects_duplicate_schema_members_from_hir() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    count: int,
    count: string
}
enum QuestProgress {
    Active { quest_id: int, quest_id: string },
    Active
}
trait Rewardable {
    fn reward(self, amount);
    fn reward(self, bonus);
}
fn main() {
    return 1;
}
"#,
    )
    .expect_err("duplicate schema members should fail before bytecode generation");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    for code in [
        "hir::duplicate_field",
        "hir::duplicate_variant",
        "hir::duplicate_variant_field",
        "hir::duplicate_trait_method",
    ] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some(code)),
            "missing diagnostic {code}: {diagnostics:?}"
        );
    }
}

#[test]
fn compiler_rejects_invalid_and_duplicate_schema_ids_from_hir() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    #[id(0)]
    zero: int
    #[id("bad")]
    bad: int
    #[id]
    missing: int
    #[id(101)]
    item_id: string
    #[id(101)]
    count: int
    #[id(102)]
    #[id(103)]
    bonus: int
}

enum QuestProgress {
    #[id(201)]
    Active {
        #[id(301)]
        count: int
        #[id(301)]
        total: int
    }
    #[id(201)]
    Started
}

fn main() {
    return 1;
}
"#,
    )
    .expect_err("invalid and duplicate schema ids should fail before bytecode generation");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    for code in [
        "hir::invalid_schema_id",
        "hir::duplicate_field_id",
        "hir::duplicate_schema_id_attr",
        "hir::duplicate_variant_id",
        "hir::duplicate_variant_field_id",
    ] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some(code)),
            "missing diagnostic {code}: {diagnostics:?}"
        );
    }
}

#[test]
fn compiler_rejects_missing_required_constructor_fields() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    item_id: string,
    count: int = 1,
}
fn main() {
    return Reward { count: 2 };
}
"#,
    )
    .expect_err("missing required constructor field should fail");
    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::missing_constructor_field"]
    );
}
#[test]
fn compiler_rejects_unknown_constructor_fields() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    item_id: string,
    count: int,
}
fn main() {
    return Reward { item_id: "gold", count: 2, bonus: 5 };
}
"#,
    )
    .expect_err("unknown constructor field should fail");
    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::unknown_constructor_field"]
    );
}
#[test]
fn compiler_rejects_duplicate_constructor_fields() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
struct Reward {
    item_id: string,
    count: int,
}
fn main() {
    return Reward { item_id: "gold", item_id: "xp", count: 2 };
}
"#,
    )
    .expect_err("duplicate constructor field should fail");
    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::duplicate_constructor_field"]
    );
}
#[test]
fn compiler_rejects_invalid_tuple_constructor_arity() {
    let missing = compile_program_source(
        SourceId::new(1),
        r#"
enum Damage {
    Magical(amount: int, element: string = "arcane"),
}
fn main() {
    return Damage::Magical();
}
"#,
    )
    .expect_err("missing tuple constructor field should fail");
    let extra = compile_program_source(
        SourceId::new(2),
        r#"
enum Damage {
    Magical(amount: int),
}
fn main() {
    return Damage::Magical(1, 2);
}
"#,
    )
    .expect_err("extra tuple constructor field should fail");
    assert_eq!(
        semantic_diagnostic_codes(missing),
        ["compiler::missing_constructor_field"]
    );
    assert_eq!(
        semantic_diagnostic_codes(extra),
        ["compiler::unknown_constructor_field"]
    );
}
#[test]
fn compiler_rejects_unknown_constructor_variants() {
    let error = compile_program_source(
        SourceId::new(1),
        r#"
enum Damage {
    Physical { amount: int },
}
fn main() {
    return Damage::Magical { amount: 7 };
}
"#,
    )
    .expect_err("unknown constructor variant should fail");
    assert_eq!(
        semantic_diagnostic_codes(error),
        ["compiler::unknown_constructor_variant"]
    );
}
#[test]
fn compiler_rejects_unresolved_names_from_hir_with_candidates() {
    let error = compile_function_source(
        SourceId::new(1),
        r#"
fn main(player) {
    return plaeyr;
}
"#,
        "main",
    )
    .expect_err("unresolved name should fail before bytecode generation");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    let unresolved = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::unresolved_name"))
        .expect("unresolved name diagnostic");
    assert_eq!(unresolved.labels.len(), 2);
    assert_eq!(unresolved.labels[0].message, "did you mean `player`?");
    assert_eq!(
        unresolved.labels[1].message,
        "candidate `player` is declared here"
    );
}
#[test]
fn compiler_rejects_unknown_schema_hints_from_hir_with_candidates() {
    let error = compile_function_source(
        SourceId::new(1),
        r#"
struct Player { level: int }
fn main(player: Plyer) {
    return 1;
}
"#,
        "main",
    )
    .expect_err("unknown schema hint should fail before bytecode generation");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    let unknown_schema = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("hir::unknown_schema"))
        .expect("unknown schema diagnostic");
    assert_eq!(unknown_schema.message, "unknown schema `Plyer`");
    assert!(
        unknown_schema
            .labels
            .iter()
            .any(|label| label.message == "candidate `Player` is declared here")
    );
}
#[test]
fn compiler_rejects_private_imports_before_codegen() {
    let error = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::main"),
            r#"
use game::reward::secret
fn main() {
    return secret();
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("game::reward"),
            r#"
fn secret() {
    return 1;
}
"#,
        ),
    ])
    .expect_err("private import should fail before bytecode generation");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::private_import"))
    );
}
#[test]
fn compiler_rejects_duplicate_imports_before_codegen() {
    let error = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::reward"),
            "pub fn grant() { return 1; }",
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("game::config"),
            "pub const BONUS = 2",
        ),
        ModuleSource::new(
            SourceId::new(3),
            ModulePath::from_qualified("game::main"),
            r#"
use game::reward::grant as reward
use game::config::BONUS as reward
fn main() {
    return reward;
}
"#,
        ),
    ])
    .expect_err("duplicate import should fail before bytecode generation");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::duplicate_import"))
    );
}
#[test]
fn compiler_rejects_import_conflicts_before_codegen() {
    let error = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_qualified("game::reward"),
            "pub fn grant() { return 1; }",
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_qualified("game::main"),
            r#"
use game::reward::grant
fn grant() {
    return 2;
}
"#,
        ),
    ])
    .expect_err("import conflict should fail before bytecode generation");
    let CompileErrorKind::SemanticDiagnostics(diagnostics) = error.kind else {
        panic!("expected semantic diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_deref() == Some("hir::import_conflict"))
    );
}
#[test]
fn compiler_keeps_valid_program_bytecode_equivalent_after_hir_gate() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
const BONUS: int = 5;
trait BonusSource { fn bonus(self) -> int; }
struct Player { level: int }
impl BonusSource for Player {
    fn bonus(self) -> int { return self.level; }
}
fn add_bonus(value) {
    return value + 5;
}
fn main() {
    let base = 10;
    return add_bonus(base) * 2;
}
"#,
    )
    .expect("valid source should compile through HIR gate");
    let main = program.function("main").expect("main function");
    assert_eq!(main.params, Vec::<String>::new());
    assert!(program.function("bonus").is_none());
    assert!(!main.instructions.is_empty());
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        UnlinkedInstructionKind::CallFunction { .. }
    )));
}
