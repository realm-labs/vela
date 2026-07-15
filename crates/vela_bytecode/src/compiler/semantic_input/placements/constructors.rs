use super::{
    CompileConstructorField, CompileConstructorTarget, CompileConstructorValue,
    CompileDynamicConstructorField, CompileError, CompileErrorKind,
    CompilePatternConstructorTarget, CompileResult, ConstructorFieldSpec, ConstructorSpec,
    ConstructorTargetFact, ContractBoundary, FunctionId, GenerationBuilder, HirBody, HirDeclId,
    HirExprId, HirExprKind, HirPatternId, HirPatternKind, MirBuildError, MirSourceOrigin,
    checked_u32, constructor_variant_specs, input_error, pattern_field_names, registry_input_error,
    require_constructor_slot_identity, unavailable_constructor_default,
};
use crate::compiler::semantic_input::schema::registry_hint_contract;

impl GenerationBuilder<'_, '_> {
    pub(super) fn insert_constructor(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
        expression: HirExprId,
    ) -> CompileResult<()> {
        let origin = self
            .expression_origin(expression)
            .ok_or_else(registry_input_error)?;
        let HirExprKind::Record {
            constructor,
            fields,
        } = &body
            .expression(expression)
            .ok_or_else(registry_input_error)?
            .kind
        else {
            return Err(input_error(MirBuildError::InconsistentInput {
                origin,
                message: format!("constructor target {expression:?} is not a record expression"),
            }));
        };
        let placement =
            self.checked_record_constructor_placement(executable, expression, fields, origin)?;
        let target = placement.target.clone();
        match target {
            ConstructorTargetFact::Declaration(declaration) => self.insert_record_constructor(
                executable,
                expression,
                declaration,
                None,
                &placement,
            ),
            ConstructorTargetFact::Variant {
                enum_declaration,
                variant,
            } => self.insert_record_constructor(
                executable,
                expression,
                enum_declaration,
                Some(&variant),
                &placement,
            ),
            ConstructorTargetFact::RegistryType { path } => self
                .insert_external_record_constructor(
                    executable, expression, &path, None, &placement,
                ),
            ConstructorTargetFact::RegistryVariant { owner, variant } => self
                .insert_external_record_constructor(
                    executable,
                    expression,
                    &owner,
                    Some(&variant),
                    &placement,
                ),
            ConstructorTargetFact::Dynamic => self.insert_dynamic_record_constructor(
                executable,
                body,
                expression,
                *constructor,
                &placement,
            ),
            ConstructorTargetFact::Unresolved => {
                Err(input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: format!(
                        "analysis did not resolve constructor target for {expression:?}"
                    ),
                }))
            }
        }
    }

    fn insert_dynamic_record_constructor(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
        expression: HirExprId,
        constructor: Option<vela_hir::ids::HirPathId>,
        placement: &vela_analysis::validation::ConstructorPlacementFact,
    ) -> CompileResult<()> {
        let origin = self
            .expression_origin(expression)
            .ok_or_else(registry_input_error)?;
        let path = constructor
            .and_then(|path| body.paths.get(&path))
            .ok_or_else(|| {
                input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: format!(
                        "dynamic constructor target {expression:?} has no authoritative HIR path"
                    ),
                })
            })?;
        if path.path.is_empty() {
            return Err(input_error(MirBuildError::InconsistentInput {
                origin,
                message: format!("dynamic constructor target {expression:?} has an empty path"),
            }));
        }
        if placement.declaration_slots.is_some() {
            return Err(input_error(MirBuildError::InconsistentInput {
                origin,
                message: "dynamic constructor unexpectedly has declaration slots".to_owned(),
            }));
        }
        let fields = placement
            .source_order
            .iter()
            .map(|source| {
                let name = source.name.clone().ok_or_else(|| {
                    input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: "dynamic record constructor source lacks a field name".to_owned(),
                    })
                })?;
                let value = source.value.ok_or_else(|| {
                    input_error(MirBuildError::InconsistentInput {
                        origin: MirSourceOrigin {
                            span: source.span,
                            ..origin
                        },
                        message: "dynamic constructor source lacks a HIR expression".to_owned(),
                    })
                })?;
                Ok(CompileDynamicConstructorField { name, value })
            })
            .collect::<CompileResult<Vec<_>>>()?;
        let target = dynamic_constructor_target(&path.path, fields, origin)?;
        self.targets
            .insert_constructor(executable, expression, target, origin)
            .map_err(input_error)
    }

    fn insert_external_record_constructor(
        &mut self,
        executable: FunctionId,
        expression: HirExprId,
        owner_name: &str,
        variant_name: Option<&str>,
        placement: &vela_analysis::validation::ConstructorPlacementFact,
    ) -> CompileResult<()> {
        let origin = self
            .expression_origin(expression)
            .ok_or_else(registry_input_error)?;
        let owner = self
            .catalog
            .type_by_source(owner_name)
            .cloned()
            .ok_or_else(|| {
                input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: format!("missing external constructor owner `{owner_name}`"),
                })
            })?;
        self.ensure_external_type(owner.id, origin)?;
        let variant = variant_name
            .map(|name| {
                self.catalog
                    .variant_by_owner_name(owner.id, name)
                    .cloned()
                    .ok_or_else(|| {
                        input_error(MirBuildError::InconsistentInput {
                            origin,
                            message: format!(
                                "missing external constructor variant `{owner_name}::{name}`"
                            ),
                        })
                    })
            })
            .transpose()?;
        if let Some(variant) = &variant {
            self.ensure_external_variant(variant.id, origin)?;
        }
        let variant_id = variant.as_ref().map(|variant| variant.id);
        let mut descriptors = self
            .catalog
            .fields_for_owner(owner.id)
            .into_iter()
            .filter(|field| field.variant == variant_id)
            .cloned()
            .collect::<Vec<_>>();
        descriptors.sort_by_key(|field| (field.declaration_order, field.path.name.clone()));
        let slots = self.constructor_slots(placement, descriptors.len(), origin)?;
        let evaluation_order = self.constructor_evaluation_order(placement, origin)?;
        let mut placed = Vec::with_capacity(slots.len());
        for (index, field) in descriptors.iter().enumerate() {
            let slot = slots
                .iter()
                .find(|slot| slot.field_name == field.path.name)
                .ok_or_else(registry_input_error)?;
            require_constructor_slot_identity(
                slot,
                usize::try_from(field.declaration_order).map_err(|_| {
                    input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: "external constructor field order exceeds usize".to_owned(),
                    })
                })?,
                &field.path.name,
                &field.path.name,
                origin,
            )?;
            self.ensure_external_field(field.id, origin)?;
            let value = match &slot.value {
                vela_analysis::validation::ConstructorSlotValueFact::Explicit {
                    source_index,
                    value,
                } => {
                    let value =
                        self.constructor_explicit_value(placement, *source_index, *value, origin)?;
                    if let Some(contract) = field
                        .type_hint
                        .as_ref()
                        .and_then(|hint| registry_hint_contract(hint, &self.catalog))
                        .and_then(crate::compiler::semantic_input::schema::meaningful_contract)
                    {
                        self.boundaries.push(ContractBoundary::field(
                            executable,
                            value,
                            contract,
                            &field.path.name,
                        ));
                    }
                    CompileConstructorValue::Explicit {
                        source_index: checked_u32(
                            *source_index,
                            origin,
                            "external constructor source field",
                        )?,
                        value,
                    }
                }
                unavailable => return Err(unavailable_constructor_default(unavailable, origin)),
            };
            placed.push(CompileConstructorField {
                field: field.id,
                parameter: checked_u32(index, origin, "external constructor field slot")?,
                parameter_name: slot.parameter_name.clone(),
                value,
            });
        }
        let target = match variant {
            Some(variant) => CompileConstructorTarget::Variant {
                type_id: owner.id,
                variant: variant.id,
                evaluation_order,
                fields: placed,
            },
            None => CompileConstructorTarget::Record {
                type_id: owner.id,
                shape: self.external_shape(owner.id).ok_or_else(|| {
                    input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: format!(
                            "external record constructor `{owner_name}` has no script-record shape"
                        ),
                    })
                })?,
                evaluation_order,
                fields: placed,
            },
        };
        self.targets
            .insert_constructor(executable, expression, target, origin)
            .map_err(input_error)
    }

    fn insert_record_constructor(
        &mut self,
        executable: FunctionId,
        expression: HirExprId,
        declaration: HirDeclId,
        variant: Option<&str>,
        placement: &vela_analysis::validation::ConstructorPlacementFact,
    ) -> CompileResult<()> {
        let type_id = self.type_ids[&declaration];
        let origin = self
            .expression_origin(expression)
            .ok_or_else(registry_input_error)?;
        let specs = self.constructor_specs(declaration, variant)?;
        let slots = self.constructor_slots(placement, specs.len(), origin)?;
        let evaluation_order = self.constructor_evaluation_order(placement, origin)?;
        let mut placed = Vec::with_capacity(slots.len());
        for (parameter, (slot, spec)) in slots.iter().zip(&specs).enumerate() {
            require_constructor_slot_identity(
                slot,
                parameter,
                &spec.field_name,
                &spec.parameter_name,
                origin,
            )?;
            let value = match &slot.value {
                vela_analysis::validation::ConstructorSlotValueFact::Explicit {
                    source_index,
                    value,
                } => {
                    let value =
                        self.constructor_explicit_value(placement, *source_index, *value, origin)?;
                    if let Some(contract) = spec.contract.clone() {
                        self.boundaries.push(ContractBoundary::field(
                            executable,
                            value,
                            contract,
                            &spec.field_name,
                        ));
                    }
                    CompileConstructorValue::Explicit {
                        source_index: checked_u32(
                            *source_index,
                            origin,
                            "constructor source field",
                        )?,
                        value,
                    }
                }
                vela_analysis::validation::ConstructorSlotValueFact::SourceDefault { body } => {
                    if spec.default_body != Some(*body) {
                        return Err(input_error(MirBuildError::InconsistentInput {
                            origin,
                            message: "analysis constructor default disagrees with HIR schema"
                                .to_owned(),
                        }));
                    }
                    self.require_evaluated_schema_default(*body)?;
                    CompileConstructorValue::EvaluatedDefault(*body)
                }
                unavailable => return Err(unavailable_constructor_default(unavailable, origin)),
            };
            placed.push(CompileConstructorField {
                field: spec.field,
                parameter: checked_u32(parameter, origin, "constructor parameter")?,
                parameter_name: spec.parameter_name.clone(),
                value,
            });
        }
        let target = if let Some(variant) = variant {
            CompileConstructorTarget::Variant {
                type_id,
                variant: self.variant_ids[&(declaration, variant.to_owned())],
                evaluation_order,
                fields: placed,
            }
        } else {
            CompileConstructorTarget::Record {
                type_id,
                shape: self.type_shapes[&type_id],
                evaluation_order,
                fields: placed,
            }
        };
        self.targets
            .insert_constructor(executable, expression, target, origin)
            .map_err(input_error)
    }

    pub(super) fn insert_variant_call_constructor(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
        expression: HirExprId,
        declaration: HirDeclId,
        variant: &str,
        call_placement: &vela_analysis::validation::CallArgumentPlacementFact,
    ) -> CompileResult<()> {
        let call = body.call(expression).ok_or_else(registry_input_error)?;
        let origin = self
            .expression_origin(expression)
            .ok_or_else(registry_input_error)?;
        let specs = self.constructor_specs(declaration, Some(variant))?;
        let expected_target = ConstructorTargetFact::Variant {
            enum_declaration: declaration,
            variant: variant.to_owned(),
        };
        let placement = self.checked_tuple_constructor_placement(
            executable,
            expression,
            &call.arguments,
            call_placement,
            &expected_target,
            origin,
        )?;
        let slots = self.constructor_slots(&placement, specs.len(), origin)?;
        let evaluation_order = self.constructor_evaluation_order(&placement, origin)?;
        let mut fields = Vec::with_capacity(slots.len());
        for (parameter, (slot, spec)) in slots.iter().zip(&specs).enumerate() {
            require_constructor_slot_identity(
                slot,
                parameter,
                &spec.field_name,
                &spec.parameter_name,
                origin,
            )?;
            let value = match &slot.value {
                vela_analysis::validation::ConstructorSlotValueFact::Explicit {
                    source_index,
                    value,
                } => {
                    let value =
                        self.constructor_explicit_value(&placement, *source_index, *value, origin)?;
                    if let Some(contract) = spec.contract.clone() {
                        self.boundaries.push(ContractBoundary::field(
                            executable,
                            value,
                            contract,
                            &spec.field_name,
                        ));
                    }
                    CompileConstructorValue::Explicit {
                        source_index: checked_u32(
                            *source_index,
                            origin,
                            "variant constructor source argument",
                        )?,
                        value,
                    }
                }
                vela_analysis::validation::ConstructorSlotValueFact::SourceDefault { body } => {
                    if spec.default_body != Some(*body) {
                        return Err(input_error(MirBuildError::InconsistentInput {
                            origin,
                            message: "analysis tuple constructor default disagrees with HIR schema"
                                .to_owned(),
                        }));
                    }
                    self.require_evaluated_schema_default(*body)?;
                    CompileConstructorValue::EvaluatedDefault(*body)
                }
                unavailable => return Err(unavailable_constructor_default(unavailable, origin)),
            };
            fields.push(CompileConstructorField {
                field: spec.field,
                parameter: checked_u32(parameter, origin, "variant constructor parameter")?,
                parameter_name: spec.parameter_name.clone(),
                value,
            });
        }
        self.targets
            .insert_constructor(
                executable,
                expression,
                CompileConstructorTarget::Variant {
                    type_id: self.type_ids[&declaration],
                    variant: self.variant_ids[&(declaration, variant.to_owned())],
                    evaluation_order,
                    fields,
                },
                origin,
            )
            .map_err(input_error)
    }

    pub(super) fn insert_registry_variant_call_constructor(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
        expression: HirExprId,
        owner: &str,
        variant: &str,
        call_placement: &vela_analysis::validation::CallArgumentPlacementFact,
    ) -> CompileResult<()> {
        let call = body.call(expression).ok_or_else(registry_input_error)?;
        let origin = self
            .expression_origin(expression)
            .ok_or_else(registry_input_error)?;
        let expected_target = ConstructorTargetFact::RegistryVariant {
            owner: owner.to_owned(),
            variant: variant.to_owned(),
        };
        let placement = self.checked_tuple_constructor_placement(
            executable,
            expression,
            &call.arguments,
            call_placement,
            &expected_target,
            origin,
        )?;
        self.insert_external_record_constructor(
            executable,
            expression,
            owner,
            Some(variant),
            &placement,
        )
    }

    fn constructor_specs(
        &self,
        declaration: HirDeclId,
        variant: Option<&str>,
    ) -> CompileResult<Vec<ConstructorSpec>> {
        let metadata = self
            .request
            .graph
            .declaration(declaration)
            .ok_or_else(registry_input_error)?;
        let fields = if let Some(variant) = variant {
            let variant = self
                .request
                .graph
                .enum_shape(declaration)
                .and_then(|shape| shape.variants.iter().find(|value| value.name == variant))
                .ok_or_else(registry_input_error)?;
            constructor_variant_specs(&variant.fields)
        } else {
            self.request
                .graph
                .struct_shape(declaration)
                .ok_or_else(registry_input_error)?
                .fields
                .iter()
                .map(ConstructorFieldSpec::from_struct)
                .collect()
        };
        Ok(fields
            .into_iter()
            .map(|field| {
                let variant_owned = variant.map(str::to_owned);
                ConstructorSpec {
                    field: self.field_ids[&(declaration, variant_owned, field.field_name.clone())],
                    parameter_name: field.parameter_name,
                    field_name: field.field_name,
                    default_body: field.default_body,
                    contract: field
                        .hint
                        .as_ref()
                        .and_then(|hint| self.type_contract_for_hint(metadata.module, hint))
                        .and_then(crate::compiler::semantic_input::schema::meaningful_contract),
                }
            })
            .collect())
    }

    pub(super) fn insert_pattern_constructor(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
        pattern: HirPatternId,
    ) -> CompileResult<()> {
        let target = self
            .executable_analysis(executable)?
            .pattern_constructor_target(pattern)
            .cloned()
            .ok_or_else(|| {
                let origin_record = body.patterns.get(&pattern);
                let origin = origin_record.map(|pattern| {
                    MirSourceOrigin::pattern(body.id, pattern.id, pattern.origin.span)
                });
                origin.map_or_else(registry_input_error, |origin| {
                    input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: format!(
                            "missing constructor analysis target for pattern {pattern:?}"
                        ),
                    })
                })
            })?;
        let origin_record = body
            .patterns
            .get(&pattern)
            .ok_or_else(registry_input_error)?;
        let origin = MirSourceOrigin::pattern(body.id, pattern, origin_record.origin.span);
        let placed = match target {
            ConstructorTargetFact::Declaration(declaration) => {
                reject_unqualified_record_pattern(body, origin_record)?;
                let type_id = self.type_ids[&declaration];
                CompilePatternConstructorTarget::NeverMatchesRecord {
                    type_id,
                    fields: pattern_field_names(origin_record)
                        .iter()
                        .map(|name| {
                            self.field_ids
                                .get(&(declaration, None, name.clone()))
                                .copied()
                                .ok_or_else(|| {
                                    input_error(MirBuildError::InconsistentInput {
                                        origin,
                                        message: format!(
                                            "missing script pattern field `{name}` for {declaration:?}"
                                        ),
                                    })
                                })
                        })
                        .collect::<CompileResult<Vec<_>>>()?,
                }
            }
            ConstructorTargetFact::Variant {
                enum_declaration,
                variant,
            } => CompilePatternConstructorTarget::Variant {
                type_id: self.type_ids[&enum_declaration],
                variant: self.variant_ids[&(enum_declaration, variant.clone())],
                fields: pattern_field_names(origin_record)
                    .iter()
                    .map(|name| {
                        self.field_ids
                            .get(&(enum_declaration, Some(variant.clone()), name.clone()))
                            .copied()
                            .ok_or_else(|| {
                                input_error(MirBuildError::InconsistentInput {
                                    origin,
                                    message: format!(
                                        "missing script variant pattern field `{name}` for {enum_declaration:?}"
                                    ),
                                })
                            })
                    })
                    .collect::<CompileResult<Vec<_>>>()?,
            },
            ConstructorTargetFact::RegistryType { path } => {
                reject_unqualified_record_pattern(body, origin_record)?;
                self.external_pattern_constructor(&path, None, origin_record, origin)?
            }
            ConstructorTargetFact::RegistryVariant { owner, variant } => self
                .external_pattern_constructor(&owner, Some(&variant), origin_record, origin)?,
            ConstructorTargetFact::Dynamic => {
                dynamic_pattern_constructor(body, origin_record, origin)?
            }
            ConstructorTargetFact::Unresolved => {
                return Err(input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: format!(
                        "analysis did not resolve pattern constructor target for {pattern:?}"
                    ),
                }));
            }
        };
        self.targets
            .insert_pattern_constructor(executable, pattern, placed, origin)
            .map_err(input_error)
    }

    fn external_pattern_constructor(
        &mut self,
        owner_name: &str,
        variant_name: Option<&str>,
        pattern: &vela_hir::body::HirPattern,
        origin: MirSourceOrigin,
    ) -> CompileResult<CompilePatternConstructorTarget> {
        let owner = self
            .catalog
            .type_by_source(owner_name)
            .cloned()
            .ok_or_else(registry_input_error)?;
        self.ensure_external_type(owner.id, origin)?;
        let variant = variant_name
            .map(|name| {
                self.catalog
                    .variant_by_owner_name(owner.id, name)
                    .cloned()
                    .ok_or_else(registry_input_error)
            })
            .transpose()?;
        if let Some(variant) = &variant {
            self.ensure_external_variant(variant.id, origin)?;
        }
        let fields = pattern_field_names(pattern)
            .into_iter()
            .map(|name| {
                self.catalog
                    .field_by_owner_name(
                        owner.id,
                        variant.as_ref().map(|variant| variant.id),
                        &name,
                    )
                    .map(|field| field.id)
                    .ok_or_else(registry_input_error)
            })
            .collect::<CompileResult<Vec<_>>>()?;
        match variant {
            Some(variant) => Ok(CompilePatternConstructorTarget::Variant {
                type_id: owner.id,
                variant: variant.id,
                fields,
            }),
            None => Ok(CompilePatternConstructorTarget::NeverMatchesRecord {
                type_id: owner.id,
                fields,
            }),
        }
    }
}

fn dynamic_constructor_target(
    path: &[String],
    fields: Vec<CompileDynamicConstructorField>,
    origin: MirSourceOrigin,
) -> CompileResult<CompileConstructorTarget> {
    let (name, owner) = path.split_last().ok_or_else(|| {
        input_error(MirBuildError::InconsistentInput {
            origin,
            message: "dynamic constructor path is empty".to_owned(),
        })
    })?;
    if owner.is_empty() {
        return Ok(CompileConstructorTarget::DynamicRecord {
            type_name: name.clone(),
            fields,
        });
    }
    Ok(CompileConstructorTarget::DynamicVariant {
        owner_name: owner.join("::"),
        variant_name: name.clone(),
        fields,
    })
}

fn dynamic_pattern_constructor(
    body: &HirBody,
    pattern: &vela_hir::body::HirPattern,
    origin: MirSourceOrigin,
) -> CompileResult<CompilePatternConstructorTarget> {
    let path = match &pattern.kind {
        HirPatternKind::Path { path }
        | HirPatternKind::TupleVariant { path, .. }
        | HirPatternKind::RecordVariant { path, .. } => *path,
        HirPatternKind::Binding { .. }
        | HirPatternKind::Wildcard
        | HirPatternKind::Literal(_)
        | HirPatternKind::Missing => None,
    }
    .and_then(|path| body.paths.get(&path))
    .ok_or_else(|| {
        input_error(MirBuildError::InconsistentInput {
            origin,
            message: format!(
                "dynamic pattern constructor target {:?} has no authoritative HIR path",
                pattern.id
            ),
        })
    })?;
    let (name, owner) = path.path.split_last().ok_or_else(|| {
        input_error(MirBuildError::InconsistentInput {
            origin,
            message: format!(
                "dynamic pattern constructor target {:?} has an empty path",
                pattern.id
            ),
        })
    })?;
    let fields = pattern_field_names(pattern);
    if owner.is_empty() {
        return Err(
            CompileError::new(CompileErrorKind::UnsupportedRecordPattern)
                .with_span(pattern.origin.span),
        );
    }
    Ok(CompilePatternConstructorTarget::DynamicVariant {
        owner_name: owner.join("::"),
        variant_name: name.clone(),
        fields,
    })
}

fn reject_unqualified_record_pattern(
    body: &HirBody,
    pattern: &vela_hir::body::HirPattern,
) -> CompileResult<()> {
    let path = match &pattern.kind {
        HirPatternKind::Path { path }
        | HirPatternKind::TupleVariant { path, .. }
        | HirPatternKind::RecordVariant { path, .. } => *path,
        HirPatternKind::Binding { .. }
        | HirPatternKind::Wildcard
        | HirPatternKind::Literal(_)
        | HirPatternKind::Missing => None,
    }
    .and_then(|path| body.paths.get(&path));
    if path.is_some_and(|path| path.path.len() == 1) {
        return Err(
            CompileError::new(CompileErrorKind::UnsupportedRecordPattern)
                .with_span(pattern.origin.span),
        );
    }
    Ok(())
}
