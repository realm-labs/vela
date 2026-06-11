use std::collections::{BTreeMap, HashMap};

use vela_common::Span;
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::HirLocalId;
use vela_syntax::ast::{BinaryOp, Expr, ExprKind, Literal, RecordField};

use super::value_types::expression_value_type;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ValueShapeFlow {
    locals: HashMap<HirLocalId, ValueShape>,
    names: HashMap<String, ValueShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ValueShape {
    Unknown,
    Scalar(String),
    Record(RecordShape),
    Array(Box<ValueShape>),
    Map {
        key: Box<ValueShape>,
        value: Box<ValueShape>,
    },
    Set(Box<ValueShape>),
    Option(Box<ValueShape>),
    Result {
        ok: Option<Box<ValueShape>>,
        err: Option<Box<ValueShape>>,
    },
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
            Self::Unknown
            | Self::Scalar(_)
            | Self::Array(_)
            | Self::Map { .. }
            | Self::Set(_)
            | Self::Option(_)
            | Self::Result { .. } => None,
        }
    }

    pub(super) fn value_type(&self) -> Option<String> {
        match self {
            Self::Unknown => None,
            Self::Scalar(type_name) => Some(type_name.clone()),
            Self::Record(_) => None,
            Self::Array(_) => Some("array".to_owned()),
            Self::Map { .. } => Some("map".to_owned()),
            Self::Set(_) => Some("set".to_owned()),
            Self::Option(_) => Some("Option".to_owned()),
            Self::Result { .. } => Some("Result".to_owned()),
        }
    }

    pub(super) fn array_element(&self) -> Option<&ValueShape> {
        match self {
            Self::Array(element) => Some(element),
            _ => None,
        }
    }

    pub(super) fn array_element_record(&self) -> Option<&RecordShape> {
        self.array_element().and_then(ValueShape::as_record)
    }

    pub(super) fn map_parts(&self) -> Option<(&ValueShape, &ValueShape)> {
        match self {
            Self::Map { key, value } => Some((key, value)),
            _ => None,
        }
    }
}

impl RecordShape {
    fn from_field_shapes(fields: impl IntoIterator<Item = (String, ValueShape)>) -> Self {
        let fields = fields
            .into_iter()
            .enumerate()
            .map(|(slot, (field, value))| {
                (
                    field,
                    RecordFieldShape {
                        slot,
                        value_type: value.value_type(),
                        value: Some(value),
                    },
                )
            })
            .collect();
        Self { fields }
    }

    pub(super) fn field_slot(&self, field: &str) -> Option<usize> {
        self.fields.get(field).map(|shape| shape.slot)
    }

    pub(super) fn field_record_shape(&self, field: &str) -> Option<&RecordShape> {
        self.fields
            .get(field)
            .and_then(|shape| shape.value.as_ref())
            .and_then(ValueShape::as_record)
    }

    pub(super) fn field_value_shape(&self, field: &str) -> Option<&ValueShape> {
        self.fields
            .get(field)
            .and_then(|shape| shape.value.as_ref())
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
        ExprKind::Literal(Literal::Null) => Some(ValueShape::Scalar("null".to_owned())),
        ExprKind::Literal(Literal::Bool(_)) => Some(ValueShape::Scalar("bool".to_owned())),
        ExprKind::Literal(Literal::Int(_)) => Some(ValueShape::Scalar("int".to_owned())),
        ExprKind::Literal(Literal::Float(_)) => Some(ValueShape::Scalar("float".to_owned())),
        ExprKind::Literal(Literal::String(_)) => Some(ValueShape::Scalar("string".to_owned())),
        ExprKind::Binary {
            op: BinaryOp::Range | BinaryOp::RangeInclusive,
            ..
        } => Some(ValueShape::Scalar("range".to_owned())),
        ExprKind::Binary { op, left, right } => binary_shape(op, left, right),
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
            if values.is_empty() {
                return Some(ValueShape::Array(Box::new(ValueShape::Unknown)));
            }
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
        ExprKind::Map(entries) => {
            if entries.is_empty() {
                return Some(ValueShape::Map {
                    key: Box::new(ValueShape::Unknown),
                    value: Box::new(ValueShape::Unknown),
                });
            }
            let mut keys = entries
                .iter()
                .map(|entry| {
                    expression_value_shape(
                        &entry.key,
                        local_shape_at_span,
                        local_shape_named,
                        local_type_at_span,
                        local_type_named,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            let key = keys.pop()?;
            if !keys.iter().all(|shape| shape == &key) {
                return None;
            }
            let values = entries
                .iter()
                .map(|entry| {
                    expression_value_shape(
                        &entry.value,
                        local_shape_at_span,
                        local_shape_named,
                        local_type_at_span,
                        local_type_named,
                    )
                })
                .collect::<Option<Vec<_>>>();
            let value = values.and_then(common_shape).unwrap_or(ValueShape::Unknown);
            Some(ValueShape::Map {
                key: Box::new(key),
                value: Box::new(value),
            })
        }
        ExprKind::Path(path) => local_shape_at_span(expr.span).or_else(|| {
            path.as_slice()
                .first()
                .and_then(|name| (path.len() == 1).then(|| local_shape_named(name)).flatten())
        }),
        ExprKind::Call { callee, args } => call_shape(
            callee,
            args,
            local_shape_at_span,
            local_shape_named,
            local_type_at_span,
            local_type_named,
        ),
        ExprKind::Field { base, name } => {
            let shape = expression_value_shape(
                base,
                local_shape_at_span,
                local_shape_named,
                local_type_at_span,
                local_type_named,
            )?;
            shape.as_record()?.field_value_shape(name).cloned()
        }
        ExprKind::Index { base, .. } => {
            let shape = expression_value_shape(
                base,
                local_shape_at_span,
                local_shape_named,
                local_type_at_span,
                local_type_named,
            )?;
            match shape {
                ValueShape::Array(element) => Some(*element),
                ValueShape::Map { value, .. } => Some(*value),
                _ => None,
            }
        }
        ExprKind::SelfValue => local_shape_at_span(expr.span).or_else(|| local_shape_named("self")),
        _ => None,
    }
}

fn binary_shape(op: &BinaryOp, left: &Expr, right: &Expr) -> Option<ValueShape> {
    match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
            Some(ValueShape::Scalar(arithmetic_shape(left, right)?))
        }
        BinaryOp::Equal
        | BinaryOp::NotEqual
        | BinaryOp::Less
        | BinaryOp::LessEqual
        | BinaryOp::Greater
        | BinaryOp::GreaterEqual
        | BinaryOp::And
        | BinaryOp::Or => Some(ValueShape::Scalar("bool".to_owned())),
        BinaryOp::Range | BinaryOp::RangeInclusive => Some(ValueShape::Scalar("range".to_owned())),
    }
}

fn arithmetic_shape(left: &Expr, right: &Expr) -> Option<String> {
    let left_float = matches!(left.kind, ExprKind::Literal(Literal::Float(_)));
    let right_float = matches!(right.kind, ExprKind::Literal(Literal::Float(_)));
    Some(if left_float || right_float {
        "float".to_owned()
    } else {
        "int".to_owned()
    })
}

fn common_shape(mut shapes: Vec<ValueShape>) -> Option<ValueShape> {
    let first = shapes.pop()?;
    shapes.iter().all(|shape| shape == &first).then_some(first)
}

fn call_shape(
    callee: &Expr,
    args: &[vela_syntax::ast::Argument],
    local_shape_at_span: &impl Fn(Span) -> Option<ValueShape>,
    local_shape_named: &impl Fn(&str) -> Option<ValueShape>,
    local_type_at_span: &impl Fn(Span) -> Option<String>,
    local_type_named: &impl Fn(&str) -> Option<String>,
) -> Option<ValueShape> {
    match &callee.kind {
        ExprKind::Path(path) => native_call_shape(
            path,
            args,
            local_shape_at_span,
            local_shape_named,
            local_type_at_span,
            local_type_named,
        ),
        ExprKind::Field { base, name } => method_call_shape(
            base,
            name,
            args,
            local_shape_at_span,
            local_shape_named,
            local_type_at_span,
            local_type_named,
        ),
        _ => None,
    }
}

fn native_call_shape(
    path: &[String],
    args: &[vela_syntax::ast::Argument],
    local_shape_at_span: &impl Fn(Span) -> Option<ValueShape>,
    local_shape_named: &impl Fn(&str) -> Option<ValueShape>,
    local_type_at_span: &impl Fn(Span) -> Option<String>,
    local_type_named: &impl Fn(&str) -> Option<String>,
) -> Option<ValueShape> {
    let [module, function] = path else {
        return None;
    };
    let first = args.first().map(|arg| {
        expression_value_shape(
            &arg.value,
            local_shape_at_span,
            local_shape_named,
            local_type_at_span,
            local_type_named,
        )
    });
    match (module.as_str(), function.as_str()) {
        ("option", "some") => Some(ValueShape::Option(Box::new(first??))),
        ("option", "none") => Some(ValueShape::Option(Box::new(ValueShape::Unknown))),
        ("option", "unwrap_or") => {
            let option = first??;
            match option {
                ValueShape::Option(value) => Some(*value),
                _ => args.get(1).and_then(|arg| {
                    expression_value_shape(
                        &arg.value,
                        local_shape_at_span,
                        local_shape_named,
                        local_type_at_span,
                        local_type_named,
                    )
                }),
            }
        }
        ("result", "ok") => Some(ValueShape::Result {
            ok: Some(Box::new(first??)),
            err: None,
        }),
        ("result", "err") => Some(ValueShape::Result {
            ok: None,
            err: Some(Box::new(first??)),
        }),
        ("result", "unwrap_or") => {
            let result = first??;
            match result {
                ValueShape::Result { ok: Some(ok), .. } => Some(*ok),
                _ => args.get(1).and_then(|arg| {
                    expression_value_shape(
                        &arg.value,
                        local_shape_at_span,
                        local_shape_named,
                        local_type_at_span,
                        local_type_named,
                    )
                }),
            }
        }
        ("set", "from_array") => {
            let array = first??;
            array
                .array_element()
                .cloned()
                .map(|element| ValueShape::Set(Box::new(element)))
        }
        ("reflect", "attrs") | ("reflect", "effects") => Some(ValueShape::Map {
            key: Box::new(ValueShape::Scalar("string".to_owned())),
            value: Box::new(ValueShape::Unknown),
        }),
        ("reflect", "field") => Some(reflect_field_record_shape()),
        ("reflect", "fields") => Some(ValueShape::Array(Box::new(reflect_field_record_shape()))),
        ("reflect", "method") => Some(reflect_method_record_shape()),
        ("reflect", "methods") => Some(ValueShape::Array(Box::new(reflect_method_record_shape()))),
        (
            "reflect",
            "exports"
            | "functions"
            | "modules"
            | "params"
            | "permissions"
            | "required_permissions"
            | "traits"
            | "types"
            | "variants",
        ) => Some(ValueShape::Array(Box::new(ValueShape::Unknown))),
        _ => None,
    }
}

fn reflect_field_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("access".to_owned(), ValueShape::Unknown),
        (
            "attrs".to_owned(),
            ValueShape::Map {
                key: Box::new(ValueShape::Scalar("string".to_owned())),
                value: Box::new(ValueShape::Unknown),
            },
        ),
        (
            "defaulted".to_owned(),
            ValueShape::Scalar("bool".to_owned()),
        ),
        ("docs".to_owned(), ValueShape::Unknown),
        ("id".to_owned(), ValueShape::Scalar("int".to_owned())),
        ("name".to_owned(), ValueShape::Scalar("string".to_owned())),
        ("origin".to_owned(), ValueShape::Scalar("string".to_owned())),
        ("owner".to_owned(), ValueShape::Scalar("string".to_owned())),
        ("source_span".to_owned(), ValueShape::Unknown),
        ("type".to_owned(), ValueShape::Unknown),
        ("writable".to_owned(), ValueShape::Scalar("bool".to_owned())),
    ]))
}

fn reflect_method_record_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("access".to_owned(), ValueShape::Unknown),
        (
            "attrs".to_owned(),
            ValueShape::Map {
                key: Box::new(ValueShape::Scalar("string".to_owned())),
                value: Box::new(ValueShape::Unknown),
            },
        ),
        ("docs".to_owned(), ValueShape::Unknown),
        ("effects".to_owned(), ValueShape::Unknown),
        ("id".to_owned(), ValueShape::Scalar("int".to_owned())),
        ("name".to_owned(), ValueShape::Scalar("string".to_owned())),
        ("origin".to_owned(), ValueShape::Scalar("string".to_owned())),
        ("owner".to_owned(), ValueShape::Scalar("string".to_owned())),
        (
            "params".to_owned(),
            ValueShape::Array(Box::new(ValueShape::Unknown)),
        ),
        ("return".to_owned(), ValueShape::Unknown),
        ("returns".to_owned(), ValueShape::Unknown),
        ("source_span".to_owned(), ValueShape::Unknown),
    ]))
}

fn method_call_shape(
    base: &Expr,
    method: &str,
    args: &[vela_syntax::ast::Argument],
    local_shape_at_span: &impl Fn(Span) -> Option<ValueShape>,
    local_shape_named: &impl Fn(&str) -> Option<ValueShape>,
    local_type_at_span: &impl Fn(Span) -> Option<String>,
    local_type_named: &impl Fn(&str) -> Option<String>,
) -> Option<ValueShape> {
    let receiver = expression_value_shape(
        base,
        local_shape_at_span,
        local_shape_named,
        local_type_at_span,
        local_type_named,
    )?;
    match method {
        "to_upper" | "to_lower" | "trim" | "trim_start" | "trim_end" | "replace" | "repeat" => {
            Some(ValueShape::Scalar("string".to_owned()))
        }
        "join" => Some(ValueShape::Scalar("string".to_owned())),
        "len" | "count" | "sum" => Some(ValueShape::Scalar("int".to_owned())),
        "has" | "contains" | "starts_with" | "ends_with" | "is_empty" | "is_none" | "is_some"
        | "is_ok" | "is_err" | "any" | "all" | "is_subset" | "is_superset" | "is_disjoint" => {
            Some(ValueShape::Scalar("bool".to_owned()))
        }
        "slice" => match receiver.value_type().as_deref() {
            Some("string") => Some(ValueShape::Scalar("string".to_owned())),
            Some("array") => Some(receiver),
            _ => None,
        },
        "parse_int" => Some(ValueShape::Option(Box::new(ValueShape::Scalar(
            "int".to_owned(),
        )))),
        "parse_float" => Some(ValueShape::Option(Box::new(ValueShape::Scalar(
            "float".to_owned(),
        )))),
        "split" | "split_whitespace" | "split_lines" => Some(ValueShape::Array(Box::new(
            ValueShape::Scalar("string".to_owned()),
        ))),
        "split_once" => Some(ValueShape::Option(Box::new(ValueShape::Array(Box::new(
            ValueShape::Scalar("string".to_owned()),
        ))))),
        "char_at" => Some(ValueShape::Option(Box::new(ValueShape::Scalar(
            "string".to_owned(),
        )))),
        "strip_prefix" | "strip_suffix" => Some(ValueShape::Option(Box::new(ValueShape::Scalar(
            "string".to_owned(),
        )))),
        "filter" => match &receiver {
            ValueShape::Array(_) | ValueShape::Map { .. } | ValueShape::Set(_) => Some(receiver),
            ValueShape::Option(value) => Some(ValueShape::Option(value.clone())),
            _ => None,
        },
        "map" => callback_return_shape(&receiver, method, args).map(|value| match receiver {
            ValueShape::Array(_) => ValueShape::Array(Box::new(value)),
            ValueShape::Set(_) => ValueShape::Set(Box::new(value)),
            ValueShape::Option(_) => ValueShape::Option(Box::new(value)),
            ValueShape::Result { err, .. } => ValueShape::Result {
                ok: Some(Box::new(value)),
                err,
            },
            _ => value,
        }),
        "map_err" => callback_return_shape(&receiver, method, args).map(|value| match receiver {
            ValueShape::Result { ok, .. } => ValueShape::Result {
                ok,
                err: Some(Box::new(value)),
            },
            _ => value,
        }),
        "and_then" => callback_return_shape(&receiver, method, args),
        "map_values" => callback_return_shape(&receiver, method, args).and_then(|value| {
            let (key, _) = receiver.map_parts()?;
            Some(ValueShape::Map {
                key: Box::new(key.clone()),
                value: Box::new(value),
            })
        }),
        "unwrap_or" => match &receiver {
            ValueShape::Option(value) => Some((**value).clone()),
            ValueShape::Result { ok: Some(ok), .. } => Some((**ok).clone()),
            _ => None,
        },
        "or_else" => callback_return_shape(&receiver, method, args),
        "ok_or" => match &receiver {
            ValueShape::Option(value) => Some(ValueShape::Result {
                ok: Some(value.clone()),
                err: args.first().map(|arg| {
                    Box::new(
                        expression_value_shape(
                            &arg.value,
                            local_shape_at_span,
                            local_shape_named,
                            local_type_at_span,
                            local_type_named,
                        )
                        .unwrap_or(ValueShape::Unknown),
                    )
                }),
            }),
            _ => None,
        },
        "to_error_option" => match &receiver {
            ValueShape::Result { err, .. } => Some(ValueShape::Option(
                err.clone().unwrap_or(Box::new(ValueShape::Unknown)),
            )),
            _ => None,
        },
        "flatten" => match &receiver {
            ValueShape::Option(value) => match value.as_ref() {
                ValueShape::Option(inner) => Some(ValueShape::Option(inner.clone())),
                _ => Some(ValueShape::Option(value.clone())),
            },
            ValueShape::Result { ok, err } => match ok.as_deref() {
                Some(ValueShape::Result { ok, err: inner_err }) => Some(ValueShape::Result {
                    ok: ok.clone(),
                    err: inner_err.clone().or_else(|| err.clone()),
                }),
                _ => Some(ValueShape::Result {
                    ok: ok.clone(),
                    err: err.clone(),
                }),
            },
            _ => None,
        },
        "find" => match &receiver {
            ValueShape::Array(element) | ValueShape::Set(element) => {
                Some(ValueShape::Option(element.clone()))
            }
            ValueShape::Map { key, value } => Some(ValueShape::Option(Box::new(
                ValueShape::Record(RecordShape::from_field_shapes([
                    ("key".to_owned(), (**key).clone()),
                    ("value".to_owned(), (**value).clone()),
                ])),
            ))),
            ValueShape::Scalar(type_name) if type_name == "string" => Some(ValueShape::Option(
                Box::new(ValueShape::Scalar("int".to_owned())),
            )),
            _ => None,
        },
        "index_of" | "last_index_of" => Some(ValueShape::Option(Box::new(ValueShape::Scalar(
            "int".to_owned(),
        )))),
        "pop" | "remove_at" => receiver
            .array_element()
            .cloned()
            .map(|element| ValueShape::Option(Box::new(element))),
        "min" | "max" => receiver
            .array_element()
            .cloned()
            .map(|element| ValueShape::Option(Box::new(element))),
        "merge" => Some(receiver),
        "union" | "intersection" | "difference" | "symmetric_difference" => Some(receiver),
        "clear" | "set" | "remove" => None,
        "group_by" => receiver
            .array_element()
            .cloned()
            .map(|element| ValueShape::Map {
                key: Box::new(ValueShape::Scalar("string".to_owned())),
                value: Box::new(ValueShape::Array(Box::new(element))),
            }),
        "sort" | "sort_by" | "reverse" | "distinct" => Some(receiver),
        "keys" => Some(ValueShape::Array(Box::new(ValueShape::Scalar(
            "string".to_owned(),
        )))),
        "values" => match &receiver {
            ValueShape::Map { value, .. } => Some(ValueShape::Array(value.clone())),
            ValueShape::Set(element) => Some(ValueShape::Array(element.clone())),
            _ => None,
        },
        "entries" => receiver.map_parts().map(|(key, value)| {
            ValueShape::Array(Box::new(ValueShape::Record(
                RecordShape::from_field_shapes([
                    ("key".to_owned(), key.clone()),
                    ("value".to_owned(), value.clone()),
                ]),
            )))
        }),
        _ => None,
    }
}

fn callback_return_shape(
    receiver: &ValueShape,
    method: &str,
    args: &[vela_syntax::ast::Argument],
) -> Option<ValueShape> {
    let lambda = args.first()?;
    let ExprKind::Lambda { params, body } = &lambda.value.kind else {
        return None;
    };
    let hints = callback_param_shapes(receiver, method, params.len())?;
    expression_value_shape(
        body,
        &|_span| None,
        &|name| {
            params
                .iter()
                .position(|param| param.name == name)
                .and_then(|index| hints.get(index).cloned().flatten())
        },
        &|_span| None,
        &|name| {
            params
                .iter()
                .position(|param| param.name == name)
                .and_then(|index| hints.get(index))
                .and_then(|shape| shape.as_ref())
                .and_then(ValueShape::value_type)
        },
    )
}

pub(super) fn callback_param_shapes(
    receiver: &ValueShape,
    method: &str,
    param_count: usize,
) -> Option<Vec<Option<ValueShape>>> {
    match receiver {
        ValueShape::Array(element) => Some(vec![Some((**element).clone())]),
        ValueShape::Set(element) => Some(vec![Some((**element).clone())]),
        ValueShape::Map { key, value } => {
            if param_count <= 1 {
                Some(vec![Some((**value).clone())])
            } else {
                Some(vec![Some((**key).clone()), Some((**value).clone())])
            }
        }
        ValueShape::Option(value) => {
            if method == "or_else" {
                Some(Vec::new())
            } else {
                Some(vec![Some((**value).clone())])
            }
        }
        ValueShape::Result { ok, err } => match method {
            "map" | "and_then" => Some(vec![ok.as_deref().cloned()]),
            "map_err" => Some(vec![err.as_deref().cloned()]),
            _ => None,
        },
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

    pub(super) fn value_shape_for_receiver_path(
        &self,
        receiver_path: &[String],
    ) -> Option<ValueShape> {
        let [receiver] = receiver_path else {
            let (field, prefix) = receiver_path.split_last()?;
            let root = prefix.first()?;
            let mut shape = self.value_shapes.name(root)?;
            for segment in prefix.iter().skip(1) {
                shape = match shape {
                    ValueShape::Record(record) => record
                        .field_record_shape(segment)
                        .cloned()
                        .map(ValueShape::Record)?,
                    _ => return None,
                };
            }
            return match shape {
                ValueShape::Record(record) => record
                    .fields
                    .get(field)
                    .and_then(|field| field.value.clone()),
                _ => None,
            };
        };
        self.value_shapes.name(receiver)
    }
}
