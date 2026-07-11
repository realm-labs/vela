//! Heavy-HIR-to-MIR construction.
//!
//! This workspace-internal crate API is not routed from any production compile
//! entrypoint. Complete-function tests can therefore exercise the real
//! semantic input boundary while Phase 2 grows by execution responsibility.

mod aggregates;
mod assignments;
mod calls;
mod closures;
mod constructors;
mod control_flow;
mod core;
mod defaults;
mod guards;
mod host;
mod literals;
mod loops;
mod operators;
mod patterns;
mod r#try;

use crate::{MirBuildError, MirFunctionOwner, MirLoweringInput, MirProgram, MirSourceOrigin};
use vela_hir::ids::HirBodyId;

pub fn build_mir(input: MirLoweringInput<'_>) -> Result<MirProgram, MirBuildError> {
    let body = input
        .graph()
        .body(input.body())
        .ok_or(MirBuildError::MissingCompilationRoot {
            function: input.function(),
            body: input.body(),
        })?;
    let origin = MirSourceOrigin::body(body.id, body.origin.span);
    let owner = match input.identity() {
        crate::CompileFunctionIdentity::Function(function) => MirFunctionOwner::Function(function),
        crate::CompileFunctionIdentity::Method(method) => MirFunctionOwner::Method(method),
    };
    let mut program = MirProgram::new(input.targets().target_table().clone());
    let root = program.reserve_function(body.id, owner.clone(), origin)?;
    let lambdas = topological_lambdas(body.id, input.targets().lambdas().cloned().collect())?;
    let mut functions = std::collections::BTreeMap::from([(body.id, root)]);

    // Reserve the explicitly topological closure before defining any body so
    // closure allocation can refer to a generation-local child function ID
    // without coupling definition order to HIR arena allocation or expression
    // traversal.
    for target in &lambdas {
        let parent = functions.get(&target.parent).copied().ok_or_else(|| {
            MirBuildError::InconsistentInput {
                origin: target.origin,
                message: format!(
                    "lambda HIR target order places parent {:?} after child {:?}",
                    target.parent, target.body
                ),
            }
        })?;
        let nested_owner = MirFunctionOwner::Lambda {
            parent,
            expression: target.expression,
        };
        let reservation = program.reserve_function(target.body, nested_owner, target.origin)?;
        if functions.insert(target.body, reservation).is_some() {
            return Err(MirBuildError::InconsistentInput {
                origin: target.origin,
                message: format!("duplicate nested HIR body target {:?}", target.body),
            });
        }
    }

    let function = core::FunctionBuilder::new_root(input, owner, functions.clone())?.build()?;
    program.define_function(root, function)?;
    for target in &lambdas {
        let reservation = functions[&target.body];
        let parent = functions[&target.parent];
        let owner = MirFunctionOwner::Lambda {
            parent,
            expression: target.expression,
        };
        let function =
            core::FunctionBuilder::new_lambda(input, owner, target, functions.clone())?.build()?;
        program.define_function(reservation, function)?;
    }
    Ok(program)
}

fn topological_lambdas(
    root: HirBodyId,
    targets: Vec<crate::CompileLambdaTarget>,
) -> Result<Vec<crate::CompileLambdaTarget>, MirBuildError> {
    let mut by_parent = std::collections::BTreeMap::<HirBodyId, Vec<_>>::new();
    let mut by_body = std::collections::BTreeMap::new();
    for target in targets {
        if target.body == root || by_body.insert(target.body, target.clone()).is_some() {
            return Err(MirBuildError::InconsistentInput {
                origin: target.origin,
                message: format!("duplicate nested HIR body target {:?}", target.body),
            });
        }
        by_parent.entry(target.parent).or_default().push(target);
    }
    for children in by_parent.values_mut() {
        children.sort_unstable_by_key(|target| {
            (
                target.origin.span.source,
                target.origin.span.start,
                target.origin.span.end,
                target.body,
            )
        });
    }

    let mut ordered = Vec::with_capacity(by_body.len());
    let mut pending = std::collections::VecDeque::from([root]);
    while let Some(parent) = pending.pop_front() {
        let Some(children) = by_parent.remove(&parent) else {
            continue;
        };
        for child in children {
            pending.push_back(child.body);
            ordered.push(child);
        }
    }
    if ordered.len() != by_body.len() {
        let target = by_parent
            .values()
            .flatten()
            .min_by_key(|target| target.body)
            .expect("unresolved lambda target must remain");
        return Err(MirBuildError::InconsistentInput {
            origin: target.origin,
            message: format!(
                "lambda HIR target {:?} has an unresolved or cyclic executable parent {:?}",
                target.body, target.parent
            ),
        });
    }
    Ok(ordered)
}
