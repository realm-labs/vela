use std::collections::{BTreeMap, HashMap};

use vela_common::Span;
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::HirLocalId;
use vela_syntax::ast::{Expr, ExprKind, RecordField};

use super::value_types::expression_value_type;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ValueShapeFlow {
    locals: HashMap<HirLocalId, ValueShape>,
    names: HashMap<String, ValueShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ValueShape {
    Record(RecordShape),
    Array(Box<ValueShape>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RecordShape {
    fields: BTreeMap<String, RecordFieldShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordFieldShape {
    slot: usize,
    value_type: Option<String>,
    value: Option<ValueShape>,
}

impl ValueShapeFlow {
    pub(super) fn local_at_span(&self, bindings: &BindingMap, span: Span) -> Option<ValueShape> {
        let BindingResolution::Local(local) = bindings.resolution_at_span(span)? else {
            return None;
        };
        self.local(*local)
    }

    pub(super) fn local(&self, local: HirLocalId) -> Option<ValueShape> {
        self.locals.get(&local).cloned()
    }

    pub(super) fn name(&self, name: &str) -> Option<ValueShape> {
        self.names.get(name).cloned()
    }

    pub(super) fn set_name(&mut self, name: impl Into<String>, shape: Option<ValueShape>) {
        let name = name.into();
        match shape {
            Some(shape) => {
                self.names.insert(name, shape);
            }
            None => {
                self.names.remove(&name);
            }
        }
    }

    pub(super) fn set_local(
        &mut self,
        local: HirLocalId,
        name: impl Into<String>,
        shape: Option<ValueShape>,
    ) {
        let name = name.into();
        match shape {
            Some(shape) => {
                self.locals.insert(local, shape.clone());
                self.names.insert(name, shape);
            }
            None => {
                self.locals.remove(&local);
                self.names.remove(&name);
            }
        }
    }
}

impl ValueShape {
    pub(super) fn as_record(&self) -> Option<&RecordShape> {
        match self {
            Self::Record(shape) => Some(shape),
            Self::Array(_) => None,
        }
    }

    pub(super) fn array_element_record(&self) -> Option<&RecordShape> {
        match self {
            Self::Array(element) => element.as_record(),
            Self::Record(_) => None,
        }
    }
}

impl RecordShape {
    pub(super) fn field_slot(&self, field: &str) -> Option<usize> {
        self.fields.get(field).map(|shape| shape.slot)
    }

    pub(super) fn field_record_shape(&self, field: &str) -> Option<&RecordShape> {
        self.fields
            .get(field)
            .and_then(|shape| shape.value.as_ref())
            .and_then(ValueShape::as_record)
    }

    pub(super) fn field_value_type(&self, field: &str) -> Option<String> {
        self.fields
            .get(field)
            .and_then(|shape| shape.value_type.clone())
    }

    fn from_fields(
        fields: &[RecordField],
        expression_shape: &impl Fn(&Expr) -> Option<ValueShape>,
        expression_type: &impl Fn(&Expr) -> Option<String>,
    ) -> Option<Self> {
        let mut field_names = fields
            .iter()
            .map(|field| field.name.clone())
            .collect::<Vec<_>>();
        field_names.sort_unstable();
        field_names.dedup();
        if field_names.is_empty() {
            return None;
        }

        let slots = field_names
            .into_iter()
            .enumerate()
            .map(|(slot, field)| (field, slot))
            .collect::<BTreeMap<_, _>>();
        let fields = fields
            .iter()
            .filter_map(|field| {
                let slot = slots.get(&field.name).copied()?;
                let value_type = field.value.as_ref().and_then(expression_type);
                let value = field.value.as_ref().and_then(expression_shape);
                Some((
                    field.name.clone(),
                    RecordFieldShape {
                        slot,
                        value_type,
                        value,
                    },
                ))
            })
            .collect();
        Some(Self { fields })
    }
}

pub(super) fn expression_value_shape(
    expr: &Expr,
    local_shape_at_span: &impl Fn(Span) -> Option<ValueShape>,
    local_shape_named: &impl Fn(&str) -> Option<ValueShape>,
    local_type_at_span: &impl Fn(Span) -> Option<String>,
    local_type_named: &impl Fn(&str) -> Option<String>,
) -> Option<ValueShape> {
    match &expr.kind {
        ExprKind::Record { path, fields } => {
            if path.len() > 1 {
                return None;
            }
            let shape = RecordShape::from_fields(
                fields,
                &|value| {
                    expression_value_shape(
                        value,
                        local_shape_at_span,
                        local_shape_named,
                        local_type_at_span,
                        local_type_named,
                    )
                },
                &|value| expression_value_type(value, local_type_at_span, local_type_named),
            )?;
            Some(ValueShape::Record(shape))
        }
        ExprKind::Array(values) => {
            let mut shapes = values
                .iter()
                .map(|value| {
                    expression_value_shape(
                        value,
                        local_shape_at_span,
                        local_shape_named,
                        local_type_at_span,
                        local_type_named,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            let first = shapes.pop()?;
            if shapes.iter().all(|shape| shape == &first) {
                Some(ValueShape::Array(Box::new(first)))
            } else {
                None
            }
        }
        ExprKind::Path(path) => local_shape_at_span(expr.span).or_else(|| {
            path.as_slice()
                .first()
                .and_then(|name| (path.len() == 1).then(|| local_shape_named(name)).flatten())
        }),
        ExprKind::Field { base, name } => {
            let shape = expression_value_shape(
                base,
                local_shape_at_span,
                local_shape_named,
                local_type_at_span,
                local_type_named,
            )?;
            shape
                .as_record()?
                .field_record_shape(name)
                .cloned()
                .map(ValueShape::Record)
        }
        ExprKind::Index { base, .. } => {
            let shape = expression_value_shape(
                base,
                local_shape_at_span,
                local_shape_named,
                local_type_at_span,
                local_type_named,
            )?;
            shape
                .array_element_record()
                .cloned()
                .map(ValueShape::Record)
        }
        ExprKind::SelfValue => local_shape_at_span(expr.span).or_else(|| local_shape_named("self")),
        _ => None,
    }
}

impl super::Compiler<'_, '_> {
    pub(super) fn value_shape_for_expr(&self, expr: &Expr) -> Option<ValueShape> {
        expression_value_shape(
            expr,
            &|span| self.value_shapes.local_at_span(self.bindings, span),
            &|name| self.value_shapes.name(name),
            &|span| self.value_types.local_at_span(self.bindings, span),
            &|name| self.value_types.name(name),
        )
    }

    pub(super) fn record_shape_for_expr(&self, expr: &Expr) -> Option<RecordShape> {
        self.value_shape_for_expr(expr)?.as_record().cloned()
    }

    pub(super) fn record_shape_for_path_root(&self, span: Span, root: &str) -> Option<RecordShape> {
        self.value_shapes
            .local_at_span(self.bindings, span)
            .or_else(|| self.value_shapes.name(root))?
            .as_record()
            .cloned()
    }

    pub(super) fn record_shape_for_index_collection(
        &self,
        collection: &Expr,
    ) -> Option<RecordShape> {
        self.value_shape_for_expr(collection)?
            .array_element_record()
            .cloned()
    }

    pub(super) fn record_field_shape_slot_for_receiver(
        &self,
        receiver: &Expr,
        field: &str,
    ) -> Option<usize> {
        self.record_shape_for_expr(receiver)?.field_slot(field)
    }

    pub(super) fn record_field_value_type_for_expr(&self, expr: &Expr) -> Option<String> {
        let ExprKind::Field { base, name } = &expr.kind else {
            return None;
        };
        self.record_shape_for_expr(base)?.field_value_type(name)
    }
}
