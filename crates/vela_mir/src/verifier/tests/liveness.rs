use super::*;

use crate::{MirLiveValue, MirLiveness};

fn only_function(program: &crate::MirProgram) -> &MirFunction {
    let mut functions = program.functions();
    let (_, function) = functions.next().expect("expected one MIR function");
    assert!(functions.next().is_none(), "expected one MIR function");
    function
}

#[test]
fn mir_liveness_tracks_join_locals_and_early_returns() {
    let program = build(
        "fn main(condition) { let value = if condition { 1 } else { 2 }; if condition { return value; } return value + 1; }",
        &["condition"],
    );
    let function = only_function(&program);
    assert!(function.liveness().is_computed());
    let value = function
        .debug_locals()
        .find_map(|(_, debug)| (debug.name == "value").then_some(debug.storage))
        .expect("value debug local");
    let return_blocks = function
        .blocks()
        .filter_map(|(block, data)| {
            matches!(
                data.terminator().map(|terminator| &terminator.kind),
                Some(MirTerminatorKind::Return(_))
            )
            .then_some(block)
        })
        .collect::<Vec<_>>();
    assert_eq!(return_blocks.len(), 2);
    assert!(return_blocks.iter().all(|block| {
        function
            .liveness()
            .block_live_in
            .get(block)
            .is_some_and(|values| values.contains(&MirLiveValue::Local(value)))
    }));
    let debug = function
        .debug_locals()
        .find_map(|(_, debug)| (debug.storage == value).then_some(debug))
        .expect("value debug metadata");
    assert!(
        return_blocks
            .iter()
            .all(|block| debug.live_region.blocks.contains(block))
    );
    verify_mir(&program).expect("join and early-return liveness verifies");
}

#[test]
fn mir_liveness_uses_operation_live_before_at_safepoints() {
    let program = build(
        "fn main(value) { let items = [value]; return value; }",
        &["value"],
    );
    let function = only_function(&program);
    let value = function.parameters()[0].storage;
    let (destination, safepoint) = function
        .statements()
        .find_map(|(_, statement)| {
            matches!(
                statement.kind,
                MirStatementKind::Allocate(MirAggregate::Array(_))
            )
            .then(|| {
                (
                    statement.destination.expect("allocation destination"),
                    statement.safepoint.expect("allocation safepoint"),
                )
            })
        })
        .expect("array allocation");
    let live = &function
        .safepoint(safepoint)
        .expect("safepoint metadata")
        .live_values;
    assert!(live.contains(&MirLiveValue::Local(value)));
    let destination = match destination {
        MirPlace::Local(local) => MirLiveValue::Local(local),
        MirPlace::Temp(temp) => MirLiveValue::Temp(temp),
    };
    assert!(
        !live.contains(&destination),
        "an operation result is not live before it is produced"
    );
    verify_mir(&program).expect("safepoint liveness verifies");
}

#[test]
fn mir_liveness_applies_iterator_edge_definitions() {
    let program = build(
        "fn main() { for value in [1] { return value; } return 0; }",
        &[],
    );
    let function = only_function(&program);
    let (header, item, next, safepoint) = function
        .blocks()
        .find_map(|(block, data)| {
            let terminator = data.terminator()?;
            let MirTerminatorKind::IteratorNext { item, next, .. } = terminator.kind else {
                return None;
            };
            Some((
                block,
                item,
                next,
                terminator.safepoint.expect("iterator safepoint"),
            ))
        })
        .expect("iterator header");
    let item = MirLiveValue::Local(item);
    assert!(
        !function.liveness().block_live_in[&header].contains(&item),
        "the yielded item is defined on the next edge"
    );
    assert!(function.liveness().block_live_out[&header].contains(&item));
    assert!(function.liveness().block_live_in[&next].contains(&item));
    assert!(
        !function
            .safepoint(safepoint)
            .expect("iterator safepoint")
            .live_values
            .contains(&item),
        "the item is not live before iterator-next produces it"
    );
    verify_mir(&program).expect("iterator edge liveness verifies");
}

#[test]
fn mir_liveness_applies_range_edge_definitions() {
    let program = build(
        "fn main() { for value in 1..3 { return value; } return 0; }",
        &[],
    );
    let function = only_function(&program);
    let (header, cursor, exhausted, item, next, done) = function
        .blocks()
        .find_map(|(block, data)| {
            let terminator = data.terminator()?;
            let MirTerminatorKind::RangeNext {
                cursor,
                exhausted,
                item,
                next,
                done,
                ..
            } = &terminator.kind
            else {
                return None;
            };
            Some((block, *cursor, *exhausted, *item, *next, *done))
        })
        .expect("range header");
    let cursor = MirLiveValue::Local(cursor);
    let exhausted = MirLiveValue::Local(exhausted);
    let item = MirLiveValue::Local(item);
    let header_live = &function.liveness().block_live_in[&header];
    assert!(header_live.contains(&cursor));
    assert!(header_live.contains(&exhausted));
    assert!(!header_live.contains(&item));
    assert!(function.liveness().block_live_in[&next].contains(&item));
    assert!(!function.liveness().block_live_in[&done].contains(&item));
    verify_mir(&program).expect("range edge liveness verifies");
}

#[test]
fn mir_liveness_verifier_rejects_missing_or_partial_computed_metadata() {
    let mut missing = terminated_function();
    missing.set_liveness(MirLiveness {
        computed: true,
        ..MirLiveness::default()
    });
    assert!(matches!(
        verify_error(&program(missing)).into_kind(),
        MirVerifyErrorKind::InvalidLivenessMetadata(_)
    ));

    let mut partial = terminated_function();
    partial.set_liveness(MirLiveness {
        block_live_in: [(partial.entry_block(), std::collections::BTreeSet::new())]
            .into_iter()
            .collect(),
        ..MirLiveness::default()
    });
    assert!(matches!(
        verify_error(&program(partial)).into_kind(),
        MirVerifyErrorKind::InvalidLivenessMetadata(_)
    ));
}
