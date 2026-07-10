use super::*;
use vela_common::Diagnostic;

pub(super) fn require_analysis_call_target(
    target: Option<&CallTargetFact>,
    expression: HirExprId,
    origin: MirSourceOrigin,
) -> CompileResult<CallTargetFact> {
    target.cloned().ok_or_else(|| {
        input_error(MirBuildError::InconsistentInput {
            origin,
            message: format!(
                "executable analysis is missing a call target for expression {expression:?}"
            ),
        })
    })
}

pub(super) fn field_is_call_callee(body: &HirBody, expression: HirExprId) -> bool {
    body.expressions.values().any(|candidate| {
        matches!(
            &candidate.kind,
            HirExprKind::Call(call) if call.callee == expression
        )
    })
}

impl GenerationBuilder<'_, '_> {
    pub(super) fn direct_declared_receiver_fact(
        &self,
        body: &HirBody,
        mut expression: HirExprId,
    ) -> Option<TypeFact> {
        loop {
            match &body.expression(expression)?.kind {
                HirExprKind::Paren {
                    expression: Some(inner),
                } => expression = *inner,
                HirExprKind::Path(_) => break,
                _ => return None,
            }
        }
        let bindings = self.request.graph.bindings_for_body(body.id)?;
        let vela_hir::binding::BindingResolution::Local(local) = bindings.resolution(expression)?
        else {
            return None;
        };
        let hint = self
            .request
            .graph
            .local_binding(*local)?
            .type_hint
            .as_ref()?;
        let module = self.request.graph.declaration(bindings.declaration)?.module;
        Some(vela_analysis::hints::type_fact_from_hint_in_module(
            self.request.graph,
            module,
            hint,
        ))
    }

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
        let diagnostic = Diagnostic::error("schema field default must be compile-time evaluable")
            .with_code("compiler::non_constant_schema_default")
            .with_span(span)
            .with_label(span, "this default is used by an omitted constructor field");
        Err(CompileError::new(CompileErrorKind::SemanticDiagnostics(
            vec![diagnostic],
        )))
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
        if let TypeFact::LogicalRecord(record) = analysis.expression(expression)? {
            return Some(record.type_id());
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

pub(super) fn type_owner_name_for_fact(fact: &TypeFact) -> Option<&str> {
    match fact {
        TypeFact::LogicalRecord(record) => Some(record.runtime_name()),
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
}

pub(super) struct ConstructorFieldSpec {
    pub(super) field_name: String,
    pub(super) parameter_name: String,
    pub(super) default_body: Option<vela_hir::ids::HirBodyId>,
    pub(super) hint: Option<HirTypeHint>,
}

impl ConstructorFieldSpec {
    pub(super) fn from_struct(field: &StructFieldHint) -> Self {
        Self {
            field_name: field.name.clone(),
            parameter_name: field.name.clone(),
            default_body: field.default_body,
            hint: field.type_hint.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_analysis_call_target_never_becomes_an_unresolved_fallback() {
        let expression = HirExprId::new(991);
        let origin = MirSourceOrigin::body(
            vela_hir::ids::HirBodyId::new(992),
            vela_common::Span::new(vela_common::SourceId::new(993), 4, 12),
        );
        let error = require_analysis_call_target(None, expression, origin)
            .expect_err("an absent total-analysis fact must fail compile-target construction");
        assert_eq!(error.span, Some(origin.span));
        assert!(matches!(
            error.kind,
            CompileErrorKind::MirInput(error)
                if matches!(
                    error.as_ref(),
                    MirBuildError::InconsistentInput { origin: actual, message }
                        if actual == &origin
                            && message == "executable analysis is missing a call target for expression HirExprId(991)"
                )
        ));

        assert_eq!(
            require_analysis_call_target(Some(&CallTargetFact::Unresolved), expression, origin)
                .expect("an explicitly analyzed unresolved path remains a language target"),
            CallTargetFact::Unresolved
        );
    }
}
