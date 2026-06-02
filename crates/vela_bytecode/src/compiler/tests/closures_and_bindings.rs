use super::*;
use crate::{FrameSlotInfo, FrameSlotKind};

fn frame_slot<'a>(code: &'a CodeObject, name: &str, kind: FrameSlotKind) -> &'a FrameSlotInfo {
    code.frame
        .slot(name, kind)
        .unwrap_or_else(|| panic!("expected {kind:?} frame slot `{name}`"))
}

#[test]
fn compiler_lowers_lambdas_with_captures() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn make_adder(base) {
    return |value| value + base;
}
fn main() {
    let add = make_adder(10);
    return add(5);
}
"#,
    )
    .expect("capturing lambda should compile");
    let make_adder = program.function("make_adder").expect("make_adder function");
    let main = program.function("main").expect("main function");
    assert!(make_adder.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::MakeClosure { code, captures, .. }
            if code.capture_count == 1 && code.params == ["value"] && captures.len() == 1
    )));
    assert!(
        main.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, InstructionKind::CallClosure { .. }))
    );
}

#[test]
fn compiler_records_frame_metadata_for_named_slots() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main(player) {
    let total = 1;
    for reward in [total] {
        let seen = reward;
    }
    match total {
        value => {
            return value;
        }
    }
}
"#,
        "main",
    )
    .expect("frame metadata source should compile");

    assert_eq!(
        frame_slot(&code, "player", FrameSlotKind::Parameter).register,
        Register(0)
    );
    assert_eq!(
        frame_slot(&code, "total", FrameSlotKind::Local).register,
        Register(1)
    );
    assert!(
        frame_slot(&code, "seen", FrameSlotKind::Local)
            .span
            .is_some()
    );
    assert!(
        frame_slot(&code, "reward", FrameSlotKind::ForBinding)
            .local
            .is_some()
    );
    assert!(
        frame_slot(&code, "value", FrameSlotKind::PatternBinding)
            .local
            .is_some()
    );
}

#[test]
fn compiler_records_lambda_frame_metadata_for_captures_and_params() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn make_adder(base) {
    let extra = 1;
    return |value| value + base + extra;
}
"#,
    )
    .expect("capturing lambda should compile");
    let make_adder = program.function("make_adder").expect("make_adder function");
    let lambda = make_adder
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            InstructionKind::MakeClosure { code, .. } => Some(code.as_ref()),
            _ => None,
        })
        .expect("lambda code object");

    assert_eq!(
        frame_slot(lambda, "base", FrameSlotKind::Capture).register,
        Register(0)
    );
    assert_eq!(
        frame_slot(lambda, "extra", FrameSlotKind::Capture).register,
        Register(1)
    );
    assert_eq!(
        frame_slot(lambda, "value", FrameSlotKind::LambdaParameter).register,
        Register(2)
    );
}
#[test]
fn compiler_lowers_nested_lambda_transitive_captures() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn make_nested(base) {
    return |amount| {
        return |bonus| base + amount + bonus;
    };
}
fn main() {
    let make = make_nested(10);
    let add = make(4);
    return add(3);
}
"#,
    )
    .expect("nested capturing lambda should compile");
    let make_nested = program
        .function("make_nested")
        .expect("make_nested function");
    assert!(make_nested.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::MakeClosure { code, captures, .. }
            if code.capture_count == 1 && code.params == ["amount"] && captures.len() == 1
    )));
}
#[test]
fn compiler_lowers_try_propagation() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
enum Result {
    Ok(value)
    Err(message)
}
fn checked(value) {
    return Result.Ok(value);
}
fn main() {
    let value = checked(10)?;
    return Result.Ok(value + 1);
}
"#,
        "main",
    )
    .expect("try propagation should compile");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, InstructionKind::TryPropagate { .. }))
    );
}
#[test]
fn compiler_lowers_range_expressions() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let values = 1..=4;
    return values;
}
"#,
        "main",
    )
    .expect("range expression should compile");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        InstructionKind::MakeRange {
            inclusive: true,
            ..
        }
    )));
}
#[test]
fn compiler_uses_hir_declarations_for_literal_const_reads() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
const BONUS: int = 5;
fn main() {
    return BONUS;
}
"#,
        "main",
    )
    .expect("literal const reads should compile through HIR declaration facts");
    let returned = code
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            InstructionKind::Return { src } => Some(src),
            _ => None,
        })
        .expect("return instruction");
    let constant = code.instructions.iter().find_map(|instruction| {
        let InstructionKind::LoadConst { dst, constant } = instruction.kind else {
            return None;
        };
        (dst == returned).then_some(constant)
    });
    assert_eq!(
        constant.map(|constant| &code.constants[constant.0]),
        Some(&Constant::Int(5))
    );
}
#[test]
fn compiler_evaluates_pure_scalar_const_expressions() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
const BASE: int = 10;
const BONUS: int = BASE + 5 * 2;
fn main() {
    return BONUS;
}
"#,
        "main",
    )
    .expect("pure scalar const expressions should compile");
    let returned = code
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            InstructionKind::Return { src } => Some(src),
            _ => None,
        })
        .expect("return instruction");
    let constant = code.instructions.iter().find_map(|instruction| {
        let InstructionKind::LoadConst { dst, constant } = instruction.kind else {
            return None;
        };
        (dst == returned).then_some(constant)
    });
    assert_eq!(
        constant.map(|constant| &code.constants[constant.0]),
        Some(&Constant::Int(20))
    );
}
#[test]
fn compiler_evaluates_imported_scalar_const_expressions_across_modules() {
    let program = compile_module_sources(&[
        ModuleSource::new(
            SourceId::new(1),
            ModulePath::from_dotted("game.main"),
            r#"
use game.tuning.BONUS as REWARD
fn main() {
    return REWARD + 1;
}
"#,
        ),
        ModuleSource::new(
            SourceId::new(2),
            ModulePath::from_dotted("game.tuning"),
            r#"
use game.base.BASE as START
pub const BONUS: int = START + 1;
"#,
        ),
        ModuleSource::new(
            SourceId::new(3),
            ModulePath::from_dotted("game.base"),
            r#"
pub const BASE: int = 4;
"#,
        ),
    ])
    .expect("imported scalar const expressions should compile across modules");
    let main = program
        .function("game.main.main")
        .expect("qualified main function");
    assert!(main.constants.contains(&Constant::Int(5)));
}
#[test]
fn compiler_uses_hir_local_bindings_for_shadowed_registers() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = 1;
    {
        let value = 2;
    }
    return value;
}
"#,
        "main",
    )
    .expect("shadowed locals should compile through HIR bindings");
    let returned = code
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            InstructionKind::Return { src } => Some(src),
            _ => None,
        })
        .expect("return instruction");
    assert_eq!(returned, Register(0));
}
#[test]
fn compiler_uses_hir_bindings_for_record_shorthand_fields() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main() {
    let value = 1;
    {
        let value = 2;
    }
    return Reward { value };
}
"#,
        "main",
    )
    .expect("record shorthand should compile through HIR bindings");
    let value_register = code
        .instructions
        .iter()
        .find_map(|instruction| match &instruction.kind {
            InstructionKind::MakeRecord { fields, .. } => fields
                .iter()
                .find_map(|(name, register)| (name == "value").then_some(*register)),
            _ => None,
        })
        .expect("record shorthand field register");
    assert_eq!(value_register, Register(0));
}
#[test]
fn compiler_uses_hir_bindings_for_match_pattern_fields() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn main(reward) {
    let amount = 100;
    match reward {
        Reward.Granted { amount } => {
            {
                let amount = 2;
            }
            return amount;
        }
        _ => {
            return 0;
        }
    }
}
"#,
        "main",
    )
    .expect("match pattern bindings should compile through HIR bindings");
    let pattern_register = code
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            InstructionKind::GetEnumField { dst, ref field, .. } if field == "amount" => Some(dst),
            _ => None,
        })
        .expect("pattern field register");
    assert!(code.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        InstructionKind::Return { src } if src == pattern_register
    )));
}
#[test]
fn compiler_uses_hir_callee_resolution_for_shadowed_function_names() {
    let code = compile_function_source(
        SourceId::new(1),
        r#"
fn helper() {
    return 1;
}
fn main() {
    let helper = 2;
    return helper();
}
"#,
        "main",
    )
    .expect("shadowed callee name should compile through HIR binding facts");
    assert!(
        code.instructions
            .iter()
            .any(|instruction| matches!(&instruction.kind, InstructionKind::CallClosure { .. }))
    );
    assert!(!code.instructions.iter().any(|instruction| matches!(
        &instruction.kind,
        InstructionKind::CallFunction { name, .. } if name == "helper"
    )));
}
#[test]
fn compiler_preserves_runtime_diagnostic_spans_for_calls_and_arithmetic() {
    let program = compile_program_source(
        SourceId::new(7),
        r#"
fn helper() {
    return 10 / 0;
}
fn main() {
    return helper();
}
"#,
    )
    .expect("diagnostic source spans should compile");
    let helper = program.function("helper").expect("helper function");
    let div_span = helper
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            InstructionKind::Div { .. } => instruction.span,
            _ => None,
        })
        .expect("division instruction span");
    assert_eq!(div_span.source, SourceId::new(7));
    let main = program.function("main").expect("main function");
    let call_span = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction.kind {
            InstructionKind::CallFunction { ref name, .. } if name == "helper" => instruction.span,
            _ => None,
        })
        .expect("script call instruction span");
    assert_eq!(call_span.source, SourceId::new(7));
}
