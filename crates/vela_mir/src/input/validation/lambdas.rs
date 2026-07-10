use std::collections::BTreeSet;

use vela_hir::ids::HirBodyId;

use crate::{MirBuildError, MirSourceOrigin, MirTypeContract};

use super::SnapshotValidator;
use super::contracts::validate_contract;

pub(super) fn validate(validator: &SnapshotValidator<'_>) -> Result<(), MirBuildError> {
    let mut symbols = BTreeSet::new();
    for ((function, body), target) in &validator.snapshot.lambdas {
        let key = (*function, *body);
        let origin = validator.retained_origin(&validator.snapshot.origins.lambdas, &key);
        let root = validator.require_root(*function, origin, "lambda target")?;
        if target.body != *body {
            return Err(validator.error(origin, "lambda target key disagrees with its HIR body"));
        }
        if target.origin != origin || origin.body != Some(*body) {
            return Err(
                validator.error(origin, "lambda target origin is not owned by its HIR body")
            );
        }
        if root.body == *body {
            return Err(validator.error(origin, "lambda target reuses its compilation-root body"));
        }
        if target.code_symbol.is_empty() {
            return Err(validator.error(origin, "lambda target has an empty code symbol"));
        }
        if !symbols.insert((*function, target.code_symbol.as_str())) {
            return Err(
                validator.error(origin, "lambda targets under one root share a code symbol")
            );
        }
        validate_parameters(validator, target, origin)?;
        validate_parent_chain(validator, *function, *body, root.body, origin)?;
    }
    Ok(())
}

fn validate_parameters(
    validator: &SnapshotValidator<'_>,
    target: &crate::CompileLambdaTarget,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    let mut parameter_ids = BTreeSet::new();
    let mut locals = BTreeSet::new();
    let mut names = BTreeSet::new();
    for parameter in &target.parameters {
        if !parameter_ids.insert(parameter.parameter) {
            return Err(validator.error(origin, "lambda target repeats a HIR parameter ID"));
        }
        if !locals.insert(parameter.local) {
            return Err(validator.error(origin, "lambda target repeats a parameter local"));
        }
        if !names.insert(parameter.name.as_str()) {
            return Err(validator.error(origin, "lambda target repeats a parameter name"));
        }
        if parameter.origin.body != Some(target.body) {
            return Err(validator.error(
                parameter.origin,
                "lambda parameter origin belongs to a different HIR body",
            ));
        }
        if let Some(contract) = &parameter.contract {
            if matches!(contract, MirTypeContract::Any) {
                return Err(validator.error(
                    parameter.origin,
                    "lambda parameter redundantly retains an Any contract",
                ));
            }
            validate_contract(
                validator,
                contract,
                parameter.origin,
                &format!("lambda parameter {:?}", parameter.parameter),
            )?;
        }
    }
    Ok(())
}

fn validate_parent_chain(
    validator: &SnapshotValidator<'_>,
    function: vela_def::FunctionId,
    body: HirBodyId,
    root: HirBodyId,
    origin: MirSourceOrigin,
) -> Result<(), MirBuildError> {
    let mut visited = BTreeSet::from([body]);
    let mut current = validator
        .snapshot
        .lambda(function, body)
        .expect("current lambda target");
    loop {
        if current.parent == root {
            return Ok(());
        }
        if !visited.insert(current.parent) {
            return Err(validator.error(origin, "lambda target parent chain contains a cycle"));
        }
        current = validator
            .snapshot
            .lambda(function, current.parent)
            .ok_or_else(|| {
                validator.error(
                    origin,
                    format!(
                        "lambda target parent {:?} is neither the root nor another lambda",
                        current.parent
                    ),
                )
            })?;
    }
}
