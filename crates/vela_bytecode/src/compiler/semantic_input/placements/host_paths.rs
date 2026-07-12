use super::{
    CompileHostIndexCapability, CompileHostPathSegment, CompileHostPathTarget, CompileResult,
    ConstantHostIndex, FunctionId, GenerationBuilder, HirBody, HirExprId, HirExprKind, HirLiteral,
    HostFieldTarget, HostPathIndexKindFact, HostPathSegmentFact, MirSourceOrigin,
    RegistryFieldTargetFact, RegistryIndexCapabilityFact, ScalarValue, TypeFact, input_error,
    registry_input_error,
};
use crate::compiler::semantic_input::schema::contract_from_fact;

impl GenerationBuilder<'_, '_> {
    pub(super) fn insert_host_path(
        &mut self,
        executable: FunctionId,
        expression: HirExprId,
    ) -> CompileResult<()> {
        let fact = self
            .executable_analysis(executable)?
            .host_path_target(expression)
            .cloned()
            .ok_or_else(registry_input_error)?;
        let origin = self
            .expression_origin(expression)
            .ok_or_else(registry_input_error)?;
        let path = self.convert_host_path(executable, fact)?;
        self.targets
            .insert_host_path(executable, expression, path, origin)
            .map_err(input_error)
    }

    pub(super) fn derived_host_index_path(
        &mut self,
        executable: FunctionId,
        body: &HirBody,
        expression: HirExprId,
    ) -> CompileResult<Option<CompileHostPathTarget>> {
        let Some(index) = body.index(expression) else {
            return Ok(None);
        };
        let Some(base) = self
            .executable_analysis(executable)?
            .host_path_target(index.receiver)
            .cloned()
        else {
            return Ok(None);
        };
        let mut path = self.convert_host_path(executable, base)?;
        let Some(
            CompileHostPathSegment::Field(field) | CompileHostPathSegment::VariantField(field),
        ) = path.segments.last()
        else {
            return Ok(None);
        };
        let capability = CompileHostIndexCapability {
            readable: field.access.readable,
            writable: field.access.writable,
            mutable: field.access.writable,
            removable: field.access.writable,
            key: self
                .executable_analysis(executable)?
                .expression(index.index)
                .and_then(|fact| {
                    contract_from_fact(
                        fact,
                        &self.registry_facts,
                        self.request.graph,
                        &self.type_ids,
                        &self.type_shapes,
                    )
                })
                .and_then(crate::compiler::semantic_input::schema::meaningful_contract),
            value: None,
        };
        let key_is_string = matches!(
            self.executable_analysis(executable)?
                .expression(index.index),
            Some(TypeFact::Primitive(vela_common::PrimitiveTag::String))
        );
        path.segments.push(if key_is_string {
            CompileHostPathSegment::DynamicKey {
                expression: index.index,
                capability,
            }
        } else {
            CompileHostPathSegment::DynamicIndex {
                expression: index.index,
                capability,
            }
        });
        Ok(Some(path))
    }

    pub(super) fn convert_host_path(
        &mut self,
        executable: FunctionId,
        fact: vela_analysis::semantic_facts::HostPathTargetFact,
    ) -> CompileResult<CompileHostPathTarget> {
        let origin = self
            .expression_origin(fact.root)
            .ok_or_else(registry_input_error)?;
        self.ensure_external_type(fact.root_type.semantic, origin)?;
        let root_type = self
            .host_type_target(fact.root_type.semantic)
            .ok_or_else(registry_input_error)?;
        let mut segments = Vec::new();
        for segment in fact.segments {
            match segment {
                HostPathSegmentFact::Field(field) => {
                    let target = self.host_field_target(&field, origin)?;
                    segments.push(if field.variant_field {
                        CompileHostPathSegment::VariantField(target)
                    } else {
                        CompileHostPathSegment::Field(target)
                    });
                }
                HostPathSegmentFact::Index {
                    expression,
                    kind,
                    capability,
                    ..
                } => {
                    let capability = self.host_index_capability(&capability, origin);
                    let constant = self.constant_host_index(executable, expression)?;
                    segments.push(match (kind, constant) {
                        (HostPathIndexKindFact::Index, Some(ConstantHostIndex::Index(value))) => {
                            CompileHostPathSegment::ConstantIndex { value, capability }
                        }
                        (HostPathIndexKindFact::Key, Some(ConstantHostIndex::Key(value))) => {
                            CompileHostPathSegment::ConstantKey { value, capability }
                        }
                        (HostPathIndexKindFact::Index, _) => CompileHostPathSegment::DynamicIndex {
                            expression,
                            capability,
                        },
                        (HostPathIndexKindFact::Key, _) => CompileHostPathSegment::DynamicKey {
                            expression,
                            capability,
                        },
                    });
                }
            }
        }
        Ok(CompileHostPathTarget {
            root: fact.root,
            root_type,
            segments,
        })
    }

    pub(super) fn host_field_target(
        &mut self,
        fact: &RegistryFieldTargetFact,
        origin: MirSourceOrigin,
    ) -> CompileResult<HostFieldTarget> {
        self.ensure_external_field(fact.semantic, origin)?;
        let owner = self
            .host_type_target(fact.owner)
            .ok_or_else(registry_input_error)?;
        Ok(HostFieldTarget {
            owner,
            semantic: fact.semantic,
            runtime: fact.host_runtime.ok_or_else(registry_input_error)?,
            access: vela_mir::CompileFieldAccess::new(
                fact.access.readable,
                fact.access.writable,
                fact.access.reflect_readable,
                fact.access.reflect_writable,
                fact.access.required_permissions.clone(),
            ),
        })
    }

    fn host_index_capability(
        &mut self,
        capability: &RegistryIndexCapabilityFact,
        origin: MirSourceOrigin,
    ) -> CompileHostIndexCapability {
        let capability = CompileHostIndexCapability {
            readable: capability.readable,
            writable: capability.writable,
            mutable: capability.addable,
            removable: capability.removable,
            key: contract_from_fact(
                &capability.key,
                &self.registry_facts,
                self.request.graph,
                &self.type_ids,
                &self.type_shapes,
            )
            .and_then(crate::compiler::semantic_input::schema::meaningful_contract),
            value: contract_from_fact(
                &capability.value,
                &self.registry_facts,
                self.request.graph,
                &self.type_ids,
                &self.type_shapes,
            )
            .and_then(crate::compiler::semantic_input::schema::meaningful_contract),
        };
        if let Some(contract) = &capability.key {
            self.remember_contract(contract, origin);
        }
        if let Some(contract) = &capability.value {
            self.remember_contract(contract, origin);
        }
        capability
    }

    fn constant_host_index(
        &self,
        executable: FunctionId,
        expression: HirExprId,
    ) -> CompileResult<Option<ConstantHostIndex>> {
        let Some(body) = self.body_for_expression(expression) else {
            return Ok(None);
        };
        let Some(record) = body.expression(expression) else {
            return Ok(None);
        };
        Ok(match &record.kind {
            HirExprKind::Literal(HirLiteral::String(value)) => {
                Some(ConstantHostIndex::Key(value.clone()))
            }
            HirExprKind::Literal(HirLiteral::Integer(_)) => {
                let Some(scalar) = self
                    .executable_analysis(executable)?
                    .literal(expression)
                    .and_then(|literal| literal.as_ref().ok())
                    .and_then(|literal| literal.scalar())
                else {
                    return Ok(None);
                };
                match scalar {
                    ScalarValue::I64(value) => {
                        u32::try_from(value).ok().map(ConstantHostIndex::Index)
                    }
                    ScalarValue::U64(value) => {
                        u32::try_from(value).ok().map(ConstantHostIndex::Index)
                    }
                    ScalarValue::I8(value) => {
                        u32::try_from(value).ok().map(ConstantHostIndex::Index)
                    }
                    ScalarValue::I16(value) => {
                        u32::try_from(value).ok().map(ConstantHostIndex::Index)
                    }
                    ScalarValue::I32(value) => {
                        u32::try_from(value).ok().map(ConstantHostIndex::Index)
                    }
                    ScalarValue::U8(value) => Some(ConstantHostIndex::Index(u32::from(value))),
                    ScalarValue::U16(value) => Some(ConstantHostIndex::Index(u32::from(value))),
                    ScalarValue::U32(value) => Some(ConstantHostIndex::Index(value)),
                    ScalarValue::F32(_) | ScalarValue::F64(_) => None,
                }
            }
            _ => None,
        })
    }
}
