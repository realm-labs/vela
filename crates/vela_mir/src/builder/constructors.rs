use std::collections::BTreeMap;

use vela_def::FieldId;
use vela_hir::ids::{HirBodyId, HirExprId};

use crate::{
    CompileConstructorField, CompileConstructorTarget, CompileConstructorValue,
    CompileDynamicConstructorField, MirAggregate, MirBuildError, MirEffect, MirImmediate,
    MirOperand, MirPlace, MirSafepoint, MirSourceOrigin, MirStatement, MirStatementKind,
    MirValueType,
};

use super::core::FunctionBuilder;

impl FunctionBuilder<'_> {
    /// Lower one constructor from the closed compile-target snapshot.
    ///
    /// Static constructors evaluate explicit values in source order, then
    /// project them into stable descriptor slots. Dynamic constructors retain
    /// their names and source field order verbatim. No name lookup happens in
    /// this layer.
    pub(super) fn lower_constructor(
        &mut self,
        expression: HirExprId,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let target = self
            .input
            .targets()
            .constructor(expression)
            .cloned()
            .ok_or_else(|| {
                self.inconsistent(origin, "constructor expression has no compile target")
            })?;

        match target {
            CompileConstructorTarget::Record {
                type_id,
                shape,
                evaluation_order,
                fields,
            } => {
                let descriptor =
                    self.input
                        .targets()
                        .type_descriptor(type_id)
                        .ok_or_else(|| {
                            self.inconsistent(
                                origin,
                                format!(
                                    "record constructor references missing type #{}",
                                    type_id.get()
                                ),
                            )
                        })?;
                if descriptor.shape != Some(shape) {
                    return Err(self.inconsistent(
                        origin,
                        "record constructor shape disagrees with its descriptor",
                    ));
                }
                let values =
                    self.lower_static_constructor_values(&evaluation_order, &fields, origin)?;
                if self.current_is_terminated()? {
                    return Ok(MirOperand::Immediate(MirImmediate::Unit));
                }
                self.append_constructor_allocation(
                    origin,
                    MirValueType::ScriptType { type_id, shape },
                    MirAggregate::Record {
                        type_id,
                        shape,
                        fields: values,
                    },
                )
            }
            CompileConstructorTarget::Variant {
                type_id,
                variant,
                evaluation_order,
                fields,
            } => {
                let owner = self
                    .input
                    .targets()
                    .type_descriptor(type_id)
                    .ok_or_else(|| {
                        self.inconsistent(
                            origin,
                            format!(
                                "variant constructor references missing type #{}",
                                type_id.get()
                            ),
                        )
                    })?;
                let descriptor = self
                    .input
                    .targets()
                    .variant_descriptor(variant)
                    .ok_or_else(|| {
                        self.inconsistent(
                            origin,
                            format!(
                                "variant constructor references missing variant #{}",
                                variant.get()
                            ),
                        )
                    })?;
                if descriptor.owner != type_id || !owner.variants.contains(&variant) {
                    return Err(self.inconsistent(
                        origin,
                        "variant constructor owner disagrees with its descriptor",
                    ));
                }
                let values =
                    self.lower_static_constructor_values(&evaluation_order, &fields, origin)?;
                if self.current_is_terminated()? {
                    return Ok(MirOperand::Immediate(MirImmediate::Unit));
                }
                self.append_constructor_allocation(
                    origin,
                    MirValueType::Enum(type_id),
                    MirAggregate::Enum {
                        type_id,
                        variant,
                        fields: values,
                    },
                )
            }
            CompileConstructorTarget::DynamicRecord { type_name, fields } => {
                let fields = self.lower_dynamic_constructor_values(&fields)?;
                if self.current_is_terminated()? {
                    return Ok(MirOperand::Immediate(MirImmediate::Unit));
                }
                self.append_constructor_allocation(
                    origin,
                    MirValueType::Dynamic,
                    MirAggregate::DynamicRecord { type_name, fields },
                )
            }
            CompileConstructorTarget::DynamicVariant {
                owner_name,
                variant_name,
                fields,
            } => {
                let fields = self.lower_dynamic_constructor_values(&fields)?;
                if self.current_is_terminated()? {
                    return Ok(MirOperand::Immediate(MirImmediate::Unit));
                }
                self.append_constructor_allocation(
                    origin,
                    MirValueType::Dynamic,
                    MirAggregate::DynamicVariant {
                        owner_name,
                        variant_name,
                        fields,
                    },
                )
            }
        }
    }

    fn lower_static_constructor_values(
        &mut self,
        evaluation_order: &[HirExprId],
        fields: &[CompileConstructorField],
        constructor_origin: MirSourceOrigin,
    ) -> Result<Vec<(FieldId, MirOperand)>, MirBuildError> {
        let fields_by_source = fields
            .iter()
            .filter_map(|field| match field.value {
                CompileConstructorValue::Explicit { source_index, .. } => {
                    Some((source_index, field.field))
                }
                CompileConstructorValue::EvaluatedDefault(_) => None,
            })
            .collect::<BTreeMap<_, _>>();
        if fields_by_source.len() != evaluation_order.len() {
            return Err(self.inconsistent(
                constructor_origin,
                "constructor evaluation order is not covered by explicit fields",
            ));
        }

        let mut evaluated = Vec::with_capacity(evaluation_order.len());
        for (source_index, expression) in evaluation_order.iter().copied().enumerate() {
            let source_index = u32::try_from(source_index).map_err(|_| {
                self.inconsistent(
                    constructor_origin,
                    "constructor evaluation order exceeds u32",
                )
            })?;
            fields_by_source.get(&source_index).ok_or_else(|| {
                self.inconsistent(
                    constructor_origin,
                    format!("constructor source index {source_index} has no field slot"),
                )
            })?;
            let value_origin = self.constructor_operand_origin(expression)?;
            let value = self.lower_expression(expression)?;
            if self.current_is_terminated()? {
                return Ok(Vec::new());
            }
            let value = if usize::try_from(source_index)
                .is_ok_and(|index| index + 1 < evaluation_order.len())
            {
                self.capture_operand(value, value_origin)?
            } else {
                value
            };
            evaluated.push(value);
        }

        let mut projected = Vec::with_capacity(fields.len());
        for field in fields {
            let descriptor = self
                .input
                .targets()
                .field_descriptor(field.field)
                .ok_or_else(|| {
                    self.inconsistent(
                        constructor_origin,
                        format!(
                            "constructor references missing field descriptor #{}",
                            field.field.get()
                        ),
                    )
                })?;
            let value = match field.value {
                CompileConstructorValue::Explicit {
                    source_index,
                    value,
                } => {
                    let source = evaluation_order
                        .get(usize::try_from(source_index).map_err(|_| {
                            self.inconsistent(
                                constructor_origin,
                                "constructor source index exceeds usize",
                            )
                        })?)
                        .ok_or_else(|| {
                            self.inconsistent(
                                constructor_origin,
                                "constructor source index is out of bounds",
                            )
                        })?;
                    if *source != value {
                        return Err(self.inconsistent(
                            constructor_origin,
                            "constructor source expression disagrees with its field slot",
                        ));
                    }
                    evaluated
                        .get(usize::try_from(source_index).map_err(|_| {
                            self.inconsistent(
                                constructor_origin,
                                "constructor source index exceeds usize",
                            )
                        })?)
                        .cloned()
                        .ok_or_else(|| {
                            self.inconsistent(
                                constructor_origin,
                                "constructor source operand was not evaluated",
                            )
                        })?
                }
                CompileConstructorValue::EvaluatedDefault(body) => self
                    .materialize_schema_default(
                        body,
                        descriptor.contract.as_ref(),
                        constructor_origin,
                    )?,
            };
            projected.push((field.field, value));
        }
        Ok(projected)
    }

    fn lower_dynamic_constructor_values(
        &mut self,
        fields: &[CompileDynamicConstructorField],
    ) -> Result<Vec<(String, MirOperand)>, MirBuildError> {
        let mut lowered = Vec::with_capacity(fields.len());
        for (index, field) in fields.iter().enumerate() {
            let origin = self.constructor_operand_origin(field.value)?;
            let value = self.lower_expression(field.value)?;
            if self.current_is_terminated()? {
                return Ok(Vec::new());
            }
            let value = if index + 1 < fields.len() {
                self.capture_operand(value, origin)?
            } else {
                value
            };
            lowered.push((field.name.clone(), value));
        }
        Ok(lowered)
    }

    fn materialize_schema_default(
        &mut self,
        body: HirBodyId,
        contract: Option<&crate::MirTypeContract>,
        origin: MirSourceOrigin,
    ) -> Result<MirOperand, MirBuildError> {
        let value = self
            .input
            .targets()
            .evaluated_schema_default(body)
            .cloned()
            .ok_or_else(|| {
                self.inconsistent(
                    origin,
                    format!("constructor references missing evaluated default {body:?}"),
                )
            })?;
        let value_type = contract
            .map(super::constants::value_type_for_contract)
            .unwrap_or_else(|| super::constants::value_type_for_evaluated_constant(&value));
        self.lower_evaluated_constant(value, value_type, origin)
    }

    fn append_constructor_allocation(
        &mut self,
        origin: MirSourceOrigin,
        value_type: MirValueType,
        aggregate: MirAggregate,
    ) -> Result<MirOperand, MirBuildError> {
        let destination = self.function.add_temp(value_type, origin);
        let safepoint = self.function.add_safepoint(MirSafepoint::new(origin));
        self.function.append_statement(
            self.current_block,
            MirStatement::new(
                origin,
                Some(MirPlace::temp(destination)),
                MirStatementKind::Allocate(aggregate),
                MirEffect::allocation(),
                Some(safepoint),
            ),
        )?;
        Ok(MirOperand::Temp(destination))
    }

    fn constructor_operand_origin(
        &self,
        expression: HirExprId,
    ) -> Result<MirSourceOrigin, MirBuildError> {
        let record = self.body.expression(expression).ok_or_else(|| {
            self.inconsistent(
                MirSourceOrigin::body(self.body.id, self.body.origin.span),
                format!("missing constructor operand expression {expression:?}"),
            )
        })?;
        Ok(MirSourceOrigin::expression(
            self.body.id,
            expression,
            record.origin.span,
        ))
    }
}

#[cfg(test)]
#[path = "tests/constructors.rs"]
mod tests;
