use std::collections::BTreeMap;

use vela_hir::body::HirBodyOwner;
use vela_hir::ids::{HirBodyId, HirDeclId, ModuleId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::EnumVariantFieldsHint;
use vela_mir::MirEvaluatedConstant;

use super::const_eval::evaluate_const_body;
use super::error::CompileResult;
use super::value_types::{RuntimeTypeFact, type_hint_value_type};

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct EvaluatedSchemaDefaults {
    record_shapes: BTreeMap<String, ConstructorShape>,
    enum_shapes: BTreeMap<(String, String), ConstructorShape>,
    evaluated_defaults: BTreeMap<HirBodyId, Option<MirEvaluatedConstant>>,
}

impl EvaluatedSchemaDefaults {
    pub(super) fn merge(&mut self, other: Self) {
        self.record_shapes.extend(other.record_shapes);
        self.enum_shapes.extend(other.enum_shapes);
        self.evaluated_defaults.extend(other.evaluated_defaults);
    }

    pub(super) fn record(&self, type_name: &str) -> Option<&ConstructorShape> {
        self.record_shapes.get(type_name)
    }

    pub(super) fn enum_variant(&self, type_name: &str, variant: &str) -> Option<&ConstructorShape> {
        self.enum_shapes
            .get(&(type_name.to_owned(), variant.to_owned()))
    }

    pub(super) fn evaluated_defaults(&self) -> &BTreeMap<HirBodyId, Option<MirEvaluatedConstant>> {
        &self.evaluated_defaults
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ConstructorShape {
    fields: Vec<ConstructorField>,
}

impl ConstructorShape {
    fn new(fields: Vec<ConstructorField>) -> Self {
        Self { fields }
    }

    pub(super) fn field_value_type(&self, name: &str) -> Option<RuntimeTypeFact> {
        self.fields
            .iter()
            .find(|field| field.name == name)
            .and_then(|field| field.value_type.clone())
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ConstructorField {
    name: String,
    value_type: Option<RuntimeTypeFact>,
}

pub(super) fn source_schema_defaults(
    graph: &ModuleGraph,
    module: ModuleId,
    type_symbols: &BTreeMap<HirDeclId, String>,
    evaluated_constants: &BTreeMap<HirDeclId, MirEvaluatedConstant>,
) -> CompileResult<EvaluatedSchemaDefaults> {
    let mut defaults = EvaluatedSchemaDefaults::default();
    let mut evaluated_defaults = BTreeMap::new();
    for body in graph.bodies() {
        let HirBodyOwner::SchemaFieldDefault(declaration) = body.owner else {
            continue;
        };
        if !type_symbols.contains_key(&declaration) {
            continue;
        }
        let Some(bindings) = graph.schema_field_default_bindings(body.id) else {
            continue;
        };
        evaluated_defaults.insert(
            body.id,
            evaluate_const_body(body, bindings, evaluated_constants)?,
        );
    }

    for declaration in module_schema_declarations(graph, module) {
        let Some(metadata) = graph.declaration(declaration) else {
            continue;
        };
        match metadata.kind {
            DeclarationKind::Struct => {
                let Some(type_name) = type_symbols.get(&declaration).cloned() else {
                    continue;
                };
                let Some(shape) = graph.struct_shape(declaration) else {
                    continue;
                };
                let fields = shape
                    .fields
                    .iter()
                    .map(|field| ConstructorField {
                        name: field.name.clone(),
                        value_type: field.type_hint.as_ref().and_then(type_hint_value_type),
                    })
                    .collect::<Vec<_>>();
                defaults
                    .record_shapes
                    .insert(type_name, ConstructorShape::new(fields));
            }
            DeclarationKind::Enum => {
                let Some(type_name) = type_symbols.get(&declaration).cloned() else {
                    continue;
                };
                let Some(shape) = graph.enum_shape(declaration) else {
                    continue;
                };
                for variant in &shape.variants {
                    let fields = enum_variant_fields(&variant.fields);
                    defaults.enum_shapes.insert(
                        (type_name.clone(), variant.name.clone()),
                        ConstructorShape::new(fields),
                    );
                }
            }
            _ => {}
        }
    }

    defaults.evaluated_defaults = evaluated_defaults;

    Ok(defaults)
}

fn module_schema_declarations(graph: &ModuleGraph, module: ModuleId) -> Vec<HirDeclId> {
    let Some(declarations) = graph.module(module) else {
        return Vec::new();
    };

    let mut schema_declarations = declarations
        .names()
        .filter_map(|name| {
            let declaration = declarations.get(name)?;
            let metadata = graph.declaration(declaration)?;
            match metadata.kind {
                DeclarationKind::Struct | DeclarationKind::Enum => Some(declaration),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    schema_declarations.sort_unstable();
    schema_declarations
}

fn enum_variant_fields(fields: &EnumVariantFieldsHint) -> Vec<ConstructorField> {
    match fields {
        EnumVariantFieldsHint::Unit => Vec::new(),
        EnumVariantFieldsHint::Tuple(fields) => fields
            .iter()
            .enumerate()
            .map(|(index, field)| ConstructorField {
                name: index.to_string(),
                value_type: field.type_hint.as_ref().and_then(type_hint_value_type),
            })
            .collect(),
        EnumVariantFieldsHint::Record(fields) => fields
            .iter()
            .map(|field| ConstructorField {
                name: field.name.clone(),
                value_type: field.type_hint.as_ref().and_then(type_hint_value_type),
            })
            .collect(),
    }
}
