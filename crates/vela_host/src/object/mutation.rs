use crate::error::{HostError, HostErrorKind, HostResult};
use crate::resolved::HostMutationOp;
use crate::target::HostTargetInstance;
use crate::value::{HostValue, add_values, div_values, mul_values, rem_values, sub_values};

pub fn mutate_host_value(
    op: HostMutationOp,
    current: &HostValue,
    rhs: &HostValue,
    target: HostTargetInstance<'_>,
) -> HostResult<HostValue> {
    let next = match op {
        HostMutationOp::Add => add_values(current, rhs),
        HostMutationOp::Sub => sub_values(current, rhs),
        HostMutationOp::Mul => mul_values(current, rhs),
        HostMutationOp::Div => div_values(current, rhs),
        HostMutationOp::Rem => rem_values(current, rhs),
        HostMutationOp::Push => None,
    };
    next.ok_or_else(|| invalid_mutation_error(op, target))
}

fn invalid_mutation_error(op: HostMutationOp, target: HostTargetInstance<'_>) -> HostError {
    let path = target.to_diagnostic_path().to_host_path();
    HostError {
        kind: match op {
            HostMutationOp::Add => HostErrorKind::InvalidAdd { path },
            HostMutationOp::Sub => HostErrorKind::InvalidSub { path },
            HostMutationOp::Mul => HostErrorKind::InvalidMul { path },
            HostMutationOp::Div => HostErrorKind::InvalidDiv { path },
            HostMutationOp::Rem => HostErrorKind::InvalidRem { path },
            HostMutationOp::Push => HostErrorKind::InvalidPush { path },
        },
        source_span: None,
    }
}
