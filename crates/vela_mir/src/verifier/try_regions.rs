use std::collections::BTreeSet;

use crate::{
    CompileTryLayoutTarget, CompileTryTarget, MirBlockId, MirEffect, MirFieldTarget, MirOperand,
    MirPlace, MirStatementKind, MirTerminatorKind,
};

use super::cfg::FunctionGraph;
use super::{FunctionVerifier, MirVerifyError, MirVerifyErrorKind};

pub(crate) fn verify(
    verifier: &FunctionVerifier<'_>,
    graph: &FunctionGraph,
) -> Result<(), MirVerifyError> {
    let mut claimed = BTreeSet::new();
    let mut invalid_blocks = BTreeSet::new();
    for root in graph.blocks() {
        let terminator = verifier
            .function
            .block(root)
            .and_then(|block| block.terminator())
            .expect("CFG analysis retains terminators");
        let MirTerminatorKind::TrySwitch {
            value,
            target,
            result,
            continuations,
            propagate,
            invalid,
            join,
        } = &terminator.kind
        else {
            continue;
        };
        verify_target(verifier, root, terminator.origin, *target)?;
        let expected = match target {
            CompileTryTarget::Expected(layout) => vec![*layout],
            CompileTryTarget::Dynamic { option, result } => vec![*option, *result],
        };
        if continuations
            .iter()
            .map(|continuation| continuation.layout)
            .collect::<Vec<_>>()
            != expected
        {
            return Err(region_error(
                verifier,
                root,
                terminator.origin,
                "try continuations do not exactly match the ordered compile target",
            ));
        }

        let region_blocks = std::iter::once(root)
            .chain(continuations.iter().map(|continuation| continuation.block))
            .chain([*propagate, *invalid])
            .collect::<Vec<_>>();
        let distinct = region_blocks.iter().copied().collect::<BTreeSet<_>>();
        if distinct.len() != region_blocks.len() || distinct.contains(join) {
            return Err(region_error(
                verifier,
                root,
                terminator.origin,
                "try region blocks and join must be distinct",
            ));
        }
        for block in &region_blocks {
            if !claimed.insert(*block) || !graph.dominates(root, *block) {
                return Err(region_error(
                    verifier,
                    root,
                    terminator.origin,
                    "try regions overlap or the root does not dominate an internal block",
                ));
            }
        }

        let expected_join_predecessors = continuations
            .iter()
            .map(|continuation| continuation.block)
            .collect::<BTreeSet<_>>();
        if graph.predecessors(*join).collect::<BTreeSet<_>>() != expected_join_predecessors {
            return Err(region_error(
                verifier,
                root,
                terminator.origin,
                "try join predecessors are not exactly its continuation blocks",
            ));
        }
        for continuation in continuations {
            require_only_predecessor(verifier, graph, root, continuation.block, terminator.origin)?;
            verify_continue(
                verifier,
                root,
                continuation.block,
                *join,
                value,
                *result,
                continuation.layout,
                terminator.origin,
            )?;
        }
        require_only_predecessor(verifier, graph, root, *propagate, terminator.origin)?;
        require_only_predecessor(verifier, graph, root, *invalid, terminator.origin)?;
        verify_propagate(verifier, root, *propagate, value, terminator.origin)?;
        verify_invalid(verifier, root, *invalid, *target, terminator.origin)?;
        invalid_blocks.insert(*invalid);
    }

    for block in graph.blocks() {
        let terminator = verifier
            .function
            .block(block)
            .and_then(|block| block.terminator())
            .expect("CFG analysis retains terminators");
        if matches!(terminator.kind, MirTerminatorKind::TryTypeMismatch { .. })
            && !invalid_blocks.contains(&block)
        {
            return Err(region_error(
                verifier,
                block,
                terminator.origin,
                "try type mismatch terminator is outside a canonical try region",
            ));
        }
    }
    Ok(())
}

fn verify_target(
    verifier: &FunctionVerifier<'_>,
    block: MirBlockId,
    origin: crate::MirSourceOrigin,
    target: CompileTryTarget,
) -> Result<(), MirVerifyError> {
    let layouts = match target {
        CompileTryTarget::Expected(layout) => vec![layout],
        CompileTryTarget::Dynamic { option, result } => {
            if option.family == result.family || option.type_id == result.type_id {
                return Err(region_error(
                    verifier,
                    block,
                    origin,
                    "dynamic try target does not contain distinct Option and Result layouts",
                ));
            }
            vec![option, result]
        }
    };
    for layout in layouts {
        let table = verifier.program.targets();
        let owner = table.type_descriptor(layout.type_id);
        let continue_variant = table.variant(layout.continue_variant);
        let break_variant = table.variant(layout.break_variant);
        let payload = table.field(layout.continue_payload);
        if owner.is_none()
            || continue_variant.is_none_or(|variant| variant.owner != layout.type_id)
            || break_variant.is_none_or(|variant| variant.owner != layout.type_id)
            || payload.is_none_or(|field| {
                field.owner != layout.type_id || field.variant != Some(layout.continue_variant)
            })
        {
            return Err(region_error(
                verifier,
                block,
                origin,
                "try layout target is inconsistent with the MIR target table",
            ));
        }
    }
    Ok(())
}

fn require_only_predecessor(
    verifier: &FunctionVerifier<'_>,
    graph: &FunctionGraph,
    root: MirBlockId,
    block: MirBlockId,
    origin: crate::MirSourceOrigin,
) -> Result<(), MirVerifyError> {
    if graph.predecessors(block).collect::<Vec<_>>() == [root] {
        Ok(())
    } else {
        Err(region_error(
            verifier,
            root,
            origin,
            "try internal block has a predecessor other than its root",
        ))
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_continue(
    verifier: &FunctionVerifier<'_>,
    root: MirBlockId,
    block: MirBlockId,
    join: MirBlockId,
    value: &MirOperand,
    result: crate::MirLocalId,
    layout: CompileTryLayoutTarget,
    origin: crate::MirSourceOrigin,
) -> Result<(), MirVerifyError> {
    let body = verifier
        .function
        .block(block)
        .expect("canonical block exists");
    let [statement] = body.statements() else {
        return Err(region_error(
            verifier,
            root,
            origin,
            "try continuation must contain exactly one payload read",
        ));
    };
    let statement = verifier
        .function
        .statement(*statement)
        .expect("canonical statement exists");
    let expected_target = MirFieldTarget::VariantSlot {
        type_id: layout.type_id,
        variant: layout.continue_variant,
        field: layout.continue_payload,
    };
    if statement.origin != origin
        || statement.destination != Some(MirPlace::Local(result))
        || statement.effect != MirEffect::may_trap()
        || statement.safepoint.is_some()
        || !matches!(
            &statement.kind,
            MirStatementKind::ReadField { receiver, target }
                if receiver == value && *target == expected_target
        )
    {
        return Err(region_error(
            verifier,
            root,
            origin,
            "try continuation payload read is not canonical",
        ));
    }
    let terminator = body.terminator().expect("canonical block is terminated");
    if terminator.origin != origin
        || terminator.effect != MirEffect::PURE
        || terminator.safepoint.is_some()
        || terminator.kind != MirTerminatorKind::Jump(join)
    {
        return Err(region_error(
            verifier,
            root,
            origin,
            "try continuation does not jump directly to its join",
        ));
    }
    Ok(())
}

fn verify_propagate(
    verifier: &FunctionVerifier<'_>,
    root: MirBlockId,
    block: MirBlockId,
    value: &MirOperand,
    origin: crate::MirSourceOrigin,
) -> Result<(), MirVerifyError> {
    let body = verifier
        .function
        .block(block)
        .expect("canonical block exists");
    let terminator = body.terminator().expect("canonical block is terminated");
    if !body.statements().is_empty()
        || terminator.origin != origin
        || terminator.effect != MirEffect::PURE
        || terminator.safepoint.is_some()
        || !matches!(&terminator.kind, MirTerminatorKind::Return(Some(result)) if result == value)
    {
        return Err(region_error(
            verifier,
            root,
            origin,
            "try propagation block must return the original operand directly",
        ));
    }
    Ok(())
}

fn verify_invalid(
    verifier: &FunctionVerifier<'_>,
    root: MirBlockId,
    block: MirBlockId,
    target: CompileTryTarget,
    origin: crate::MirSourceOrigin,
) -> Result<(), MirVerifyError> {
    let body = verifier
        .function
        .block(block)
        .expect("canonical block exists");
    let terminator = body.terminator().expect("canonical block is terminated");
    if !body.statements().is_empty()
        || terminator.origin != origin
        || terminator.effect != MirEffect::may_trap()
        || terminator.safepoint.is_some()
        || terminator.kind != (MirTerminatorKind::TryTypeMismatch { target })
    {
        return Err(region_error(
            verifier,
            root,
            origin,
            "try invalid block is not the canonical type-mismatch edge",
        ));
    }
    Ok(())
}

fn region_error(
    verifier: &FunctionVerifier<'_>,
    block: MirBlockId,
    origin: crate::MirSourceOrigin,
    detail: &str,
) -> MirVerifyError {
    verifier.error(
        Some(block),
        None,
        origin,
        MirVerifyErrorKind::InvalidTerminatorContract(detail.to_owned()),
    )
}
