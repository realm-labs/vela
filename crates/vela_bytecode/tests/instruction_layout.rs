//! Guards the layout facts the linked dispatch loop is tuned against.

use vela_bytecode::linked::{Instruction, InstructionKind};

/// The interpreter walks `code.instructions` as a flat slice, so the stride of
/// one `Instruction` sets the access pattern of every dispatch step.
///
/// A power-of-two, cache-line-aligned stride measured materially faster than
/// the natural 120-byte layout these fields happen to produce: restoring it
/// took `scalar_branch_loop` from +21.5% to +12.5% against its pre-change
/// baseline while improving `function_calls` and `recursive_countdown`
/// further. The alignment is deliberate rather than incidental.
///
/// This is a tuning guard, not a correctness rule, but partial slimming does
/// not help and has been tried. Moving `mir_origin` and `mir_budget_charges`
/// (24 bytes of verification-only data the interpreter never reads) into a
/// side table takes the struct to 96 bytes. At `align(32)` that stride
/// measured slower on nine rows, worst `range_iteration` +11.0% and
/// `scalar_branch_loop` +7.7%, because 96 is not a power of two: indexing
/// stops being a shift and instructions straddle cache lines. Padding the
/// smaller struct back to 128 measured neutral, which means the saving is
/// entirely absorbed and buys neither time nor memory.
///
/// Reaching the next useful stride, 64, needs both halves at once:
/// `InstructionKind` is 72 bytes on its own, so it has to shrink below about
/// 56 *and* `span` has to leave the struct. Either change alone is wasted
/// work.
#[test]
fn instruction_stride_stays_cache_line_aligned() {
    assert_eq!(
        std::mem::size_of::<Instruction>(),
        128,
        "instruction stride changed; re-measure the dispatch loop before accepting it"
    );
    assert_eq!(
        std::mem::align_of::<Instruction>(),
        64,
        "instruction alignment changed; re-measure the dispatch loop before accepting it"
    );
}

/// Keeps the operand payload from growing unnoticed. `InstructionKind` is the
/// dominant term in the stride above.
#[test]
fn instruction_kind_payload_stays_bounded() {
    assert!(
        std::mem::size_of::<InstructionKind>() <= 72,
        "instruction operand payload grew to {} bytes; it feeds the dispatch stride",
        std::mem::size_of::<InstructionKind>()
    );
}
