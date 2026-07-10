use super::*;

pub(super) fn field_is_call_callee(body: &HirBody, expression: HirExprId) -> bool {
    body.expressions.values().any(|candidate| {
        matches!(
            &candidate.kind,
            HirExprKind::Call(call) if call.callee == expression
        )
    })
}

impl GenerationBuilder<'_, '_> {
    pub(super) fn require_evaluated_schema_default(
        &self,
        body: vela_hir::ids::HirBodyId,
    ) -> CompileResult<()> {
        if self
            .request
            .schema_defaults
            .evaluated_defaults()
            .get(&body)
            .is_some_and(Option::is_some)
        {
            return Ok(());
        }
        let span = self
            .request
            .graph
            .body(body)
            .map(|body| body.origin.span)
            .ok_or_else(registry_input_error)?;
        Err(CompileError::new(CompileErrorKind::UnsupportedSyntax(
            "non-constant schema default expression",
        ))
        .with_span(span))
    }

    pub(super) fn collect_typed_let_boundaries(&mut self, executable: FunctionId, body: &HirBody) {
        let module = self
            .request
            .graph
            .bindings_for_body(body.id)
            .and_then(|bindings| self.request.graph.declaration(bindings.declaration))
            .map(|declaration| declaration.module);
        let Some(module) = module else {
            return;
        };
        for statement in body.statements.values() {
            let vela_hir::body::HirStmtKind::Let {
                pattern: Some(pattern),
                type_hint: Some(hint),
                initializer: Some(initializer),
            } = &statement.kind
            else {
                continue;
            };
            let Some(contract) = self.type_contract_for_hint(module, hint) else {
                continue;
            };
            let name = body
                .patterns
                .get(pattern)
                .and_then(|pattern| pattern.local())
                .and_then(|local| self.request.graph.local_binding(local))
                .map(|local| local.name.clone())
                .unwrap_or_else(|| "local".to_owned());
            self.boundaries.push(ContractBoundary::typed_let(
                executable,
                *initializer,
                contract,
                name,
            ));
        }
    }

    pub(super) fn owner_type_for_expression(
        &self,
        executable: FunctionId,
        expression: HirExprId,
    ) -> Option<TypeId> {
        let analysis = self.executable_analysis(executable).ok()?;
        if let Some(script) = analysis.script_type(expression) {
            return self.type_ids.get(&script.declaration).copied();
        }
        let name = type_owner_name_for_fact(analysis.expression(expression)?)?;
        self.registry_facts
            .type_target_fact(name)
            .map(|target| target.semantic)
            .or_else(|| vela_stdlib::std_type_id(name))
            .or_else(|| {
                self.type_names.iter().find_map(|(id, candidate)| {
                    (candidate == name || candidate.ends_with(&format!("::{name}"))).then_some(*id)
                })
            })
    }

    pub(super) fn external_shape(&self, owner: TypeId) -> Option<vela_common::ShapeId> {
        let ty = self.catalog.ty(owner)?;
        if ty.kind != vela_registry::TypeKindDef::ScriptStruct {
            return None;
        }
        let name = super::super::external::source_name(&ty.path);
        let mut fields = self
            .catalog
            .fields_for_owner(owner)
            .into_iter()
            .filter(|field| field.variant.is_none())
            .map(|field| field.path.name.as_str())
            .collect::<Vec<_>>();
        fields.sort_unstable();
        Some(vela_common::script_shape_id(&name, fields.into_iter()))
    }
}

pub(super) fn hir_call_arguments(arguments: &[HirArgument]) -> CompileResult<Vec<HirCallArgument>> {
    arguments
        .iter()
        .map(|argument| {
            Ok(HirCallArgument {
                name: argument.name.clone(),
                span: argument.origin.span,
                value: argument.value.ok_or_else(|| {
                    CompileError::new(CompileErrorKind::UnsupportedSyntax("call argument"))
                        .with_span(argument.origin.span)
                })?,
            })
        })
        .collect()
}

pub(super) fn positional_values(arguments: &[HirArgument]) -> CompileResult<Vec<HirExprId>> {
    arguments
        .iter()
        .map(|argument| {
            argument.value.ok_or_else(|| {
                CompileError::new(CompileErrorKind::UnsupportedSyntax("call argument"))
                    .with_span(argument.origin.span)
            })
        })
        .collect()
}

pub(super) fn dynamic_values(
    arguments: &[HirArgument],
) -> CompileResult<Vec<CompileDynamicCallArgument>> {
    arguments
        .iter()
        .map(|argument| {
            Ok(CompileDynamicCallArgument {
                name: argument.name.clone(),
                value: argument.value.ok_or_else(|| {
                    CompileError::new(CompileErrorKind::UnsupportedSyntax("call argument"))
                        .with_span(argument.origin.span)
                })?,
            })
        })
        .collect()
}

pub(super) fn callee_path(body: &HirBody, callee: HirExprId) -> Option<&[String]> {
    body.paths
        .iter()
        .find(|path| {
            path.kind == HirPathKind::Callee && path.owner == HirPathOwner::Expression(callee)
        })
        .map(|path| path.path.as_slice())
}

pub(super) fn reflection_operation(path: &str) -> Option<CompileReflectionCall> {
    match path {
        "reflect::get" => Some(CompileReflectionCall::Read),
        "reflect::set" => Some(CompileReflectionCall::Write),
        "reflect::call" => Some(CompileReflectionCall::Call),
        _ => None,
    }
}

pub(super) fn semantic_diagnostics(diagnostics: Vec<vela_common::Diagnostic>) -> CompileError {
    CompileError::new(CompileErrorKind::SemanticDiagnostics(diagnostics))
}

pub(super) fn checked_u32(
    value: usize,
    origin: MirSourceOrigin,
    description: &str,
) -> CompileResult<u32> {
    u32::try_from(value).map_err(|_| {
        input_error(MirBuildError::InconsistentInput {
            origin,
            message: format!("{description} exceeds u32::MAX"),
        })
    })
}

pub(super) fn checked_u16(
    value: usize,
    origin: MirSourceOrigin,
    description: &str,
) -> CompileResult<u16> {
    u16::try_from(value).map_err(|_| {
        input_error(MirBuildError::InconsistentInput {
            origin,
            message: format!("{description} exceeds u16::MAX"),
        })
    })
}

pub(super) fn type_owner_name_for_fact(fact: &TypeFact) -> Option<&str> {
    match fact {
        TypeFact::Record { name } | TypeFact::Enum { name, .. } | TypeFact::Host { name } => {
            Some(name)
        }
        fact => type_owner_name_standard(fact),
    }
}

pub(super) fn type_owner_name_standard(fact: &TypeFact) -> Option<&'static str> {
    match fact {
        TypeFact::Primitive(PrimitiveTag::Unit) => Some("Unit"),
        TypeFact::Primitive(PrimitiveTag::Bool) => Some("bool"),
        TypeFact::Primitive(PrimitiveTag::I8) => Some("i8"),
        TypeFact::Primitive(PrimitiveTag::I16) => Some("i16"),
        TypeFact::Primitive(PrimitiveTag::I32) => Some("i32"),
        TypeFact::Primitive(PrimitiveTag::I64) => Some("i64"),
        TypeFact::Primitive(PrimitiveTag::U8) => Some("u8"),
        TypeFact::Primitive(PrimitiveTag::U16) => Some("u16"),
        TypeFact::Primitive(PrimitiveTag::U32) => Some("u32"),
        TypeFact::Primitive(PrimitiveTag::U64) => Some("u64"),
        TypeFact::Primitive(PrimitiveTag::F32) => Some("f32"),
        TypeFact::Primitive(PrimitiveTag::F64) => Some("f64"),
        TypeFact::Primitive(PrimitiveTag::Char) => Some("char"),
        TypeFact::Primitive(PrimitiveTag::String) => Some("String"),
        TypeFact::Primitive(PrimitiveTag::Bytes) => Some("Bytes"),
        TypeFact::Array { .. } => Some("Array"),
        TypeFact::Map { .. } => Some("Map"),
        TypeFact::Set { .. } => Some("Set"),
        TypeFact::Iterator { .. } => Some("Iterator"),
        TypeFact::Range => Some("Range"),
        TypeFact::Option { .. } | TypeFact::OptionSome { .. } | TypeFact::OptionNone => {
            Some("Option")
        }
        TypeFact::Result { .. } | TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. } => {
            Some("Result")
        }
        TypeFact::Function { .. } => Some("Function"),
        TypeFact::Closure => Some("Closure"),
        _ => None,
    }
}

pub(super) fn type_owner_name(fact: Option<&TypeFact>) -> Option<&str> {
    type_owner_name_for_fact(fact?)
}

pub(super) enum ConstantHostIndex {
    Index(u32),
    Key(String),
}

pub(super) struct ConstructorSpec {
    pub(super) field: FieldId,
    pub(super) field_name: String,
    pub(super) parameter_name: String,
    pub(super) default_body: Option<vela_hir::ids::HirBodyId>,
    pub(super) contract: Option<vela_mir::MirTypeContract>,
    pub(super) hint: Option<HirTypeHint>,
    pub(super) span: vela_common::Span,
}

pub(super) struct ConstructorFieldSpec {
    pub(super) field_name: String,
    pub(super) parameter_name: String,
    pub(super) default_body: Option<vela_hir::ids::HirBodyId>,
    pub(super) hint: Option<HirTypeHint>,
    pub(super) span: vela_common::Span,
}

impl ConstructorFieldSpec {
    pub(super) fn from_struct(field: &StructFieldHint) -> Self {
        Self {
            field_name: field.name.clone(),
            parameter_name: field.name.clone(),
            default_body: field.default_body,
            hint: field.type_hint.clone(),
            span: field.span,
        }
    }
}

pub(super) fn constructor_variant_specs(
    fields: &EnumVariantFieldsHint,
) -> Vec<ConstructorFieldSpec> {
    match fields {
        EnumVariantFieldsHint::Unit => Vec::new(),
        EnumVariantFieldsHint::Tuple(fields) => fields
            .iter()
            .enumerate()
            .map(|(index, field)| ConstructorFieldSpec {
                field_name: index.to_string(),
                parameter_name: field.name.clone(),
                default_body: field.default_body,
                hint: field.type_hint.clone(),
                span: field.span,
            })
            .collect(),
        EnumVariantFieldsHint::Record(fields) => fields
            .iter()
            .map(ConstructorFieldSpec::from_struct)
            .collect(),
    }
}

pub(super) fn pattern_field_names(pattern: &vela_hir::body::HirPattern) -> Vec<String> {
    match &pattern.kind {
        HirPatternKind::TupleVariant { fields, .. } => {
            (0..fields.len()).map(|index| index.to_string()).collect()
        }
        HirPatternKind::RecordVariant { fields, .. } => {
            fields.iter().map(|field| field.name.clone()).collect()
        }
        HirPatternKind::Path { .. }
        | HirPatternKind::Binding { .. }
        | HirPatternKind::Wildcard
        | HirPatternKind::Literal(_)
        | HirPatternKind::Missing => Vec::new(),
    }
}
