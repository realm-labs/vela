use super::{FunctionVerifier, MirVerifyError, MirVerifyErrorKind};

pub(crate) fn verify(verifier: &FunctionVerifier<'_>) -> Result<(), MirVerifyError> {
    let function = verifier.function;
    if !function.liveness().is_computed() {
        let has_partial_liveness = !function.liveness().block_live_in.is_empty()
            || !function.liveness().block_live_out.is_empty()
            || !function.liveness().statement_live_before.is_empty()
            || !function.liveness().statement_live_after.is_empty()
            || function
                .safepoints()
                .any(|(_, safepoint)| !safepoint.live_values.is_empty())
            || function
                .debug_locals()
                .any(|(_, debug)| !debug.live_region.blocks.is_empty());
        return if has_partial_liveness {
            Err(error(
                verifier,
                "uncomputed liveness carries partial live metadata",
            ))
        } else {
            Ok(())
        };
    }

    let expected = crate::liveness::analyze(function);
    if function.liveness() != &expected.liveness {
        return Err(error(
            verifier,
            "stored block or statement liveness disagrees with the CFG",
        ));
    }
    for (id, safepoint) in function.safepoints() {
        if expected.safepoints.get(&id) != Some(&safepoint.live_values) {
            return Err(error(
                verifier,
                "safepoint live values are not the operation live-before set",
            ));
        }
    }
    for (id, debug) in function.debug_locals() {
        if expected.debug_regions.get(&id) != Some(&debug.live_region) {
            return Err(error(
                verifier,
                "debug local live region disagrees with local liveness",
            ));
        }
    }
    Ok(())
}

fn error(verifier: &FunctionVerifier<'_>, detail: &str) -> MirVerifyError {
    verifier.error(
        None,
        None,
        verifier.function.origin(),
        MirVerifyErrorKind::InvalidLivenessMetadata(detail.to_owned()),
    )
}
