use super::*;

fn returned_constant(code: &UnlinkedCodeObject) -> Option<&Constant> {
    let mut register = code.instructions.iter().rev().find_map(|instruction| {
        let UnlinkedInstructionKind::Return { src } = instruction.kind else {
            return None;
        };
        Some(src)
    })?;

    for instruction in code.instructions.iter().rev() {
        match instruction.kind {
            UnlinkedInstructionKind::Move { dst, src } if dst == register => register = src,
            UnlinkedInstructionKind::LoadConst { dst, constant } if dst == register => {
                return code.constants.get(constant.0);
            }
            _ => {}
        }
    }
    None
}

#[test]
fn production_compile_respects_terminated_block_tail_semantics() {
    let program = compile_program_source(
        SourceId::new(1),
        r#"
fn terminated() {
    return { 9; };
}

fn nonterminated() {
    return { 9 };
}

fn defaulted(value = { 9; }) {
    return value;
}
"#,
    )
    .expect("block-tail fixture should compile");

    let terminated = program
        .function("terminated")
        .expect("terminated function should exist");
    assert_eq!(returned_constant(terminated), Some(&Constant::Unit));
    assert!(terminated.constants.contains(&Constant::i64(9)));

    let nonterminated = program
        .function("nonterminated")
        .expect("nonterminated function should exist");
    assert_eq!(returned_constant(nonterminated), Some(&Constant::i64(9)));
    assert!(!nonterminated.constants.contains(&Constant::Unit));

    let defaulted = program
        .function("defaulted")
        .expect("defaulted function should exist");
    assert!(defaulted.constants.contains(&Constant::i64(9)));
    assert!(defaulted.constants.contains(&Constant::Unit));
}
