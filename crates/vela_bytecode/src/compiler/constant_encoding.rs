//! Physical bytecode encoding for backend-neutral evaluated constants.
//!
//! Compile-time const and schema evaluation stores `MirEvaluatedConstant`.
//! This module is the one-way boundary that turns those logical values into
//! bytecode constant-pool entries for physical backends.

use vela_mir::MirEvaluatedConstant;

use crate::Constant;

pub(super) fn encode_evaluated_constant(value: &MirEvaluatedConstant) -> Constant {
    match value {
        MirEvaluatedConstant::Unit => Constant::Unit,
        MirEvaluatedConstant::Bool(value) => Constant::Bool(*value),
        MirEvaluatedConstant::Char(value) => Constant::Char(*value),
        MirEvaluatedConstant::Scalar(value) => Constant::Scalar(*value),
        MirEvaluatedConstant::String(value) => Constant::String(value.clone()),
        MirEvaluatedConstant::Bytes(value) => Constant::Bytes(value.clone()),
        MirEvaluatedConstant::Array(values) => {
            Constant::Array(values.iter().map(encode_evaluated_constant).collect())
        }
        MirEvaluatedConstant::Map(entries) => Constant::Map(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), encode_evaluated_constant(value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use vela_common::ScalarValue;

    use super::*;

    #[test]
    fn evaluated_aggregate_constants_cross_the_physical_boundary_recursively() {
        let value = MirEvaluatedConstant::Map(vec![(
            "items".to_owned(),
            MirEvaluatedConstant::Array(vec![MirEvaluatedConstant::Scalar(ScalarValue::I64(7))]),
        )]);

        assert_eq!(
            encode_evaluated_constant(&value),
            Constant::Map(vec![(
                "items".to_owned(),
                Constant::Array(vec![Constant::Scalar(ScalarValue::I64(7))]),
            )])
        );
    }
}
