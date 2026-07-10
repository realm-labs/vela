use super::*;

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
        let target = self
            .executable_analysis(executable)?
            .constructor_target(expression)
            .cloned()
            .ok_or_else(|| {
                input_error(MirBuildError::InconsistentInput {
                    origin,
                    message: format!("missing analysis constructor target for {expression:?}"),
                })
            })?;
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
        match target {
            ConstructorTargetFact::Declaration(declaration) => {
                self.insert_record_constructor(executable, expression, declaration, None, fields)
            }
            ConstructorTargetFact::Variant {
                enum_declaration,
                variant,
            } => self.insert_record_constructor(
                executable,
                expression,
                enum_declaration,
                Some(&variant),
                fields,
            ),
            ConstructorTargetFact::RegistryType { path } => {
                self.insert_external_record_constructor(executable, expression, &path, None, fields)
            }
            ConstructorTargetFact::RegistryVariant { owner, variant } => self
                .insert_external_record_constructor(
                    executable,
                    expression,
                    &owner,
                    Some(&variant),
                    fields,
                ),
            ConstructorTargetFact::Dynamic => self.insert_dynamic_record_constructor(
                executable,
                body,
                expression,
                *constructor,
                fields,
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
        fields: &[HirRecordField],
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
        let uses = fields
            .iter()
            .map(|field| ConstructorFieldUse {
                name: field.name.clone(),
                span: field.name_origin.span,
            })
            .collect::<Vec<_>>();
        let display_name = path.path.join("::");
        let diagnostics =
            record_constructor_field_diagnostics(&display_name, None, &uses, origin.span);
        if !diagnostics.is_empty() {
            return Err(semantic_diagnostics(diagnostics));
        }
        let fields = fields
            .iter()
            .map(|field| {
                let value = field.value.ok_or_else(|| {
                    CompileError::new(CompileErrorKind::UnsupportedSyntax("record field"))
                        .with_span(field.name_origin.span)
                })?;
                Ok(CompileDynamicConstructorField {
                    name: field.name.clone(),
                    value,
                })
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
        fields: &[HirRecordField],
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
        let mut placed = Vec::with_capacity(fields.len());
        for source_field in fields {
            let field = self
                .catalog
                .field_by_owner_name(
                    owner.id,
                    variant.as_ref().map(|variant| variant.id),
                    &source_field.name,
                )
                .cloned()
                .ok_or_else(|| {
                    input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: format!(
                            "external constructor field `{owner_name}::{}` has no descriptor",
                            source_field.name
                        ),
                    })
                })?;
            self.ensure_external_field(field.id, origin)?;
            let value = source_field.value.ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnsupportedSyntax("record field"))
                    .with_span(source_field.name_origin.span)
            })?;
            if let Some(contract) = field
                .type_hint
                .as_ref()
                .and_then(|hint| registry_hint_contract(hint, &self.catalog))
                .and_then(super::super::schema::meaningful_contract)
            {
                self.boundaries.push(ContractBoundary::field(
                    executable,
                    value,
                    contract,
                    &source_field.name,
                ));
            }
            placed.push(CompileConstructorField {
                field: field.id,
                parameter: field.declaration_order,
                parameter_name: source_field.name.clone(),
                value: CompileConstructorValue::Explicit(value),
            });
        }
        let target = match variant {
            Some(variant) => CompileConstructorTarget::Variant {
                type_id: owner.id,
                variant: variant.id,
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
        fields: &[HirRecordField],
    ) -> CompileResult<()> {
        let type_id = self.type_ids[&declaration];
        let type_name = self.type_names[&type_id].clone();
        let origin = self
            .expression_origin(expression)
            .ok_or_else(registry_input_error)?;
        let specs = self.constructor_specs(declaration, variant)?;
        let uses = fields
            .iter()
            .map(|field| ConstructorFieldUse {
                name: field.name.clone(),
                span: field.name_origin.span,
            })
            .collect::<Vec<_>>();
        let shape = variant.map_or_else(
            || self.request.schema_defaults.record(&type_name),
            |variant| {
                self.request
                    .schema_defaults
                    .enum_variant(&type_name, variant)
            },
        );
        let diagnostics = record_constructor_field_diagnostics(
            &variant.map_or_else(
                || type_name.clone(),
                |variant| format!("{type_name}::{variant}"),
            ),
            shape,
            &uses,
            origin.span,
        );
        if !diagnostics.is_empty() {
            return Err(semantic_diagnostics(diagnostics));
        }
        let explicit = fields
            .iter()
            .filter_map(|field| field.value.map(|value| (field.name.as_str(), value)))
            .collect::<BTreeMap<_, _>>();
        let mut placed = Vec::new();
        for (parameter, spec) in specs.iter().enumerate() {
            let value = match explicit.get(spec.field_name.as_str()).copied() {
                Some(value) => {
                    if let Some(contract) = spec.contract.clone() {
                        self.boundaries.push(ContractBoundary::field(
                            executable,
                            value,
                            contract,
                            &spec.field_name,
                        ));
                    }
                    CompileConstructorValue::Explicit(value)
                }
                None => {
                    let body = spec.default_body.ok_or_else(registry_input_error)?;
                    self.require_evaluated_schema_default(body)?;
                    CompileConstructorValue::EvaluatedDefault(body)
                }
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
                fields: placed,
            }
        } else {
            CompileConstructorTarget::Record {
                type_id,
                shape: self.type_shapes[&type_id],
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
    ) -> CompileResult<()> {
        let call = body.call(expression).ok_or_else(registry_input_error)?;
        let origin = self
            .expression_origin(expression)
            .ok_or_else(registry_input_error)?;
        let specs = self.constructor_specs(declaration, Some(variant))?;
        let params = specs
            .iter()
            .map(|spec| ParamHint {
                name: spec.parameter_name.clone(),
                span: spec.span,
                type_hint: spec.hint.clone(),
                default_value_span: spec.default_body.map(|_| spec.span),
                default_body: spec.default_body,
            })
            .collect::<Vec<_>>();
        let arguments = hir_call_arguments(&call.arguments)?;
        let slots = resolve_hir_call_arguments(&params, &arguments, origin.span)
            .map_err(semantic_diagnostics)?;
        let mut fields = Vec::new();
        for (parameter, (slot, spec)) in slots.into_iter().zip(&specs).enumerate() {
            let value = match slot {
                Some(value) => {
                    if let Some(contract) = spec.contract.clone() {
                        self.boundaries.push(ContractBoundary::field(
                            executable,
                            value.value,
                            contract,
                            &spec.field_name,
                        ));
                    }
                    CompileConstructorValue::Explicit(value.value)
                }
                None => {
                    let body = spec.default_body.ok_or_else(registry_input_error)?;
                    self.require_evaluated_schema_default(body)?;
                    CompileConstructorValue::EvaluatedDefault(body)
                }
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
                    fields,
                },
                origin,
            )
            .map_err(input_error)
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
                        .and_then(super::super::schema::meaningful_contract),
                    hint: field.hint,
                    span: field.span,
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
                let type_id = self.type_ids[&declaration];
                let shape = self.type_shapes.get(&type_id).copied().ok_or_else(|| {
                    input_error(MirBuildError::InconsistentInput {
                        origin,
                        message: format!(
                            "record pattern declaration {declaration:?} has no shape"
                        ),
                    })
                })?;
                CompilePatternConstructorTarget::Record {
                    type_id,
                    shape,
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
            ConstructorTargetFact::RegistryType { path } => self
                .external_pattern_constructor(&path, None, origin_record, origin)?,
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
            None => Ok(CompilePatternConstructorTarget::Record {
                type_id: owner.id,
                shape: self
                    .external_shape(owner.id)
                    .ok_or_else(registry_input_error)?,
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
        return Ok(CompilePatternConstructorTarget::DynamicRecord {
            type_name: name.clone(),
            fields,
        });
    }
    Ok(CompilePatternConstructorTarget::DynamicVariant {
        owner_name: owner.join("::"),
        variant_name: name.clone(),
        fields,
    })
}
