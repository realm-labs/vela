use std::collections::{BTreeMap, BTreeSet, HashMap};

use vela_common::{PrimitiveTag, Span};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::HirLocalId;
use vela_syntax::ast::{BinaryOp, Expr, ExprKind, Literal, RecordField};

use crate::compiler::body_payloads::CompilerExpressionPayload;

use super::record_reflection_shapes;
use super::value_types::{RuntimeTypeFact, StandardRuntimeType, expression_value_type};

mod syntax_shapes;

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
    Iterator(Box<ValueShape>),
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
    type_name: Option<String>,
    fields: BTreeMap<String, RecordFieldShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordFieldShape {
    slot: usize,
    value_type: Option<RuntimeTypeFact>,
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
        if let Some(shape) = shape {
            self.names.insert(name, shape);
        } else {
            self.names.remove(&name);
        }
    }

    pub(super) fn set_local(
        &mut self,
        local: HirLocalId,
        name: impl Into<String>,
        shape: Option<ValueShape>,
    ) {
        let name = name.into();
        if let Some(shape) = shape {
            self.locals.insert(local, shape.clone());
            self.names.insert(name, shape);
        } else {
            self.locals.remove(&local);
            self.names.remove(&name);
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
            | Self::Iterator(_)
            | Self::Map { .. }
            | Self::Set(_)
            | Self::Option(_)
            | Self::Result { .. } => None,
        }
    }

    pub(super) fn value_type(&self) -> Option<RuntimeTypeFact> {
        match self {
            Self::Unknown => None,
            Self::Scalar(type_name) => scalar_shape_type_fact(type_name),
            Self::Record(_) => None,
            Self::Array(element) => element
                .value_type()
                .map(RuntimeTypeFact::array)
                .or_else(|| Some(RuntimeTypeFact::standard(StandardRuntimeType::Array))),
            Self::Iterator(item) => item
                .value_type()
                .map(RuntimeTypeFact::iterator)
                .or_else(|| Some(RuntimeTypeFact::standard(StandardRuntimeType::Iterator))),
            Self::Map { key, value } => match (key.value_type(), value.value_type()) {
                (Some(key), Some(value)) => Some(RuntimeTypeFact::map(key, value)),
                _ => Some(RuntimeTypeFact::standard(StandardRuntimeType::Map)),
            },
            Self::Set(element) => element
                .value_type()
                .map(RuntimeTypeFact::set)
                .or_else(|| Some(RuntimeTypeFact::standard(StandardRuntimeType::Set))),
            Self::Option(value) => value
                .value_type()
                .map(|payload| RuntimeTypeFact::Option(Box::new(payload)))
                .or_else(|| Some(RuntimeTypeFact::standard(StandardRuntimeType::Option))),
            Self::Result { ok, err } => match (
                ok.as_deref().and_then(ValueShape::value_type),
                err.as_deref().and_then(ValueShape::value_type),
            ) {
                (Some(ok), Some(err)) => Some(RuntimeTypeFact::Result {
                    ok: Box::new(ok),
                    err: Box::new(err),
                }),
                _ => Some(RuntimeTypeFact::standard(StandardRuntimeType::Result)),
            },
        }
    }

    pub(super) fn from_runtime_type(fact: RuntimeTypeFact) -> Self {
        let type_name = match fact {
            RuntimeTypeFact::Primitive(PrimitiveTag::Null) => "null",
            RuntimeTypeFact::Primitive(PrimitiveTag::Bool) => "bool",
            RuntimeTypeFact::Primitive(PrimitiveTag::Char) => "char",
            RuntimeTypeFact::Primitive(PrimitiveTag::I8) => "i8",
            RuntimeTypeFact::Primitive(PrimitiveTag::I16) => "i16",
            RuntimeTypeFact::Primitive(PrimitiveTag::I32) => "i32",
            RuntimeTypeFact::Primitive(PrimitiveTag::I64) => "i64",
            RuntimeTypeFact::Primitive(PrimitiveTag::U8) => "u8",
            RuntimeTypeFact::Primitive(PrimitiveTag::U16) => "u16",
            RuntimeTypeFact::Primitive(PrimitiveTag::U32) => "u32",
            RuntimeTypeFact::Primitive(PrimitiveTag::U64) => "u64",
            RuntimeTypeFact::Primitive(PrimitiveTag::F32) => "f32",
            RuntimeTypeFact::Primitive(PrimitiveTag::F64) => "f64",
            RuntimeTypeFact::Primitive(PrimitiveTag::String) => "String",
            RuntimeTypeFact::Primitive(PrimitiveTag::Bytes) => "Bytes",
            RuntimeTypeFact::Standard(StandardRuntimeType::Array) => {
                return Self::Array(Box::new(Self::Unknown));
            }
            RuntimeTypeFact::Array(element) => {
                return Self::Array(Box::new(Self::from_runtime_type(*element)));
            }
            RuntimeTypeFact::Standard(StandardRuntimeType::Map) => {
                return Self::Map {
                    key: Box::new(Self::Unknown),
                    value: Box::new(Self::Unknown),
                };
            }
            RuntimeTypeFact::Map { key, value } => {
                return Self::Map {
                    key: Box::new(Self::from_runtime_type(*key)),
                    value: Box::new(Self::from_runtime_type(*value)),
                };
            }
            RuntimeTypeFact::Standard(StandardRuntimeType::Set) => {
                return Self::Set(Box::new(Self::Unknown));
            }
            RuntimeTypeFact::Set(element) => {
                return Self::Set(Box::new(Self::from_runtime_type(*element)));
            }
            RuntimeTypeFact::Standard(StandardRuntimeType::Range) => "Range",
            RuntimeTypeFact::Standard(StandardRuntimeType::Function) => "Function",
            RuntimeTypeFact::Standard(StandardRuntimeType::Closure) => "Closure",
            RuntimeTypeFact::Standard(StandardRuntimeType::Iterator) => {
                return Self::Iterator(Box::new(Self::Unknown));
            }
            RuntimeTypeFact::Iterator(item) => {
                return Self::Iterator(Box::new(Self::from_runtime_type(*item)));
            }
            RuntimeTypeFact::Standard(StandardRuntimeType::Option) => {
                return Self::Option(Box::new(Self::Unknown));
            }
            RuntimeTypeFact::Standard(StandardRuntimeType::Result) => {
                return Self::Result {
                    ok: None,
                    err: None,
                };
            }
            RuntimeTypeFact::Option(payload) => {
                return Self::Option(Box::new(Self::from_runtime_type(*payload)));
            }
            RuntimeTypeFact::Result { ok, err } => {
                return Self::Result {
                    ok: Some(Box::new(Self::from_runtime_type(*ok))),
                    err: Some(Box::new(Self::from_runtime_type(*err))),
                };
            }
        };
        Self::Scalar(type_name.to_owned())
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

    fn iterator_item(&self) -> Option<&ValueShape> {
        match self {
            Self::Iterator(item) => Some(item),
            _ => None,
        }
    }

    pub(super) fn map_parts(&self) -> Option<(&ValueShape, &ValueShape)> {
        match self {
            Self::Map { key, value } => Some((key, value)),
            _ => None,
        }
    }

    pub(super) fn map_entry(key: ValueShape, value: ValueShape) -> Self {
        Self::Record(RecordShape::from_field_shapes_with_type(
            Some("MapEntry".to_owned()),
            [("key".to_owned(), key), ("value".to_owned(), value)],
        ))
    }
}

fn scalar_shape_type_fact(type_name: &str) -> Option<RuntimeTypeFact> {
    match type_name {
        "Null" | "null" => Some(RuntimeTypeFact::primitive(PrimitiveTag::Null)),
        "Bool" | "bool" => Some(RuntimeTypeFact::primitive(PrimitiveTag::Bool)),
        "I8" | "i8" => Some(RuntimeTypeFact::primitive(PrimitiveTag::I8)),
        "I16" | "i16" => Some(RuntimeTypeFact::primitive(PrimitiveTag::I16)),
        "I32" | "i32" => Some(RuntimeTypeFact::primitive(PrimitiveTag::I32)),
        "I64" | "i64" => Some(RuntimeTypeFact::primitive(PrimitiveTag::I64)),
        "U8" | "u8" => Some(RuntimeTypeFact::primitive(PrimitiveTag::U8)),
        "U16" | "u16" => Some(RuntimeTypeFact::primitive(PrimitiveTag::U16)),
        "U32" | "u32" => Some(RuntimeTypeFact::primitive(PrimitiveTag::U32)),
        "U64" | "u64" => Some(RuntimeTypeFact::primitive(PrimitiveTag::U64)),
        "F32" | "f32" => Some(RuntimeTypeFact::primitive(PrimitiveTag::F32)),
        "F64" | "f64" => Some(RuntimeTypeFact::primitive(PrimitiveTag::F64)),
        "String" => Some(RuntimeTypeFact::primitive(PrimitiveTag::String)),
        "Bytes" => Some(RuntimeTypeFact::primitive(PrimitiveTag::Bytes)),
        "Range" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Range)),
        "Function" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Function)),
        "Closure" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Closure)),
        "Iterator" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Iterator)),
        _ => None,
    }
}

impl RecordShape {
    pub(super) fn from_field_shapes(
        fields: impl IntoIterator<Item = (String, ValueShape)>,
    ) -> Self {
        Self::from_field_shapes_with_type(None, fields)
    }

    fn from_field_shapes_with_type(
        type_name: Option<String>,
        fields: impl IntoIterator<Item = (String, ValueShape)>,
    ) -> Self {
        let fields = fields.into_iter().collect::<BTreeMap<_, _>>();
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
        Self { type_name, fields }
    }

    pub(super) fn type_name(&self) -> Option<&str> {
        self.type_name.as_deref()
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

    pub(super) fn field_value_type(&self, field: &str) -> Option<RuntimeTypeFact> {
        self.fields
            .get(field)
            .and_then(|shape| shape.value_type.clone())
    }

    fn from_fields(
        type_name: Option<String>,
        fields: &[RecordField],
        expression_shape: &impl Fn(&Expr) -> Option<ValueShape>,
        expression_type: &impl Fn(&Expr) -> Option<RuntimeTypeFact>,
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
        Some(Self { type_name, fields })
    }
}

pub(super) fn expression_value_shape(
    expr: &Expr,
    local_shape_at_span: &impl Fn(Span) -> Option<ValueShape>,
    local_shape_named: &impl Fn(&str) -> Option<ValueShape>,
    local_type_at_span: &impl Fn(Span) -> Option<RuntimeTypeFact>,
    local_type_named: &impl Fn(&str) -> Option<RuntimeTypeFact>,
) -> Option<ValueShape> {
    match &expr.kind {
        ExprKind::Literal(_) => expression_value_type(expr, local_type_at_span, local_type_named)
            .map(ValueShape::from_runtime_type),
        ExprKind::InterpolatedString(_) => Some(ValueShape::Scalar("String".to_owned())),
        ExprKind::Binary {
            op: BinaryOp::Range | BinaryOp::RangeInclusive,
            ..
        } => Some(ValueShape::Scalar("Range".to_owned())),
        ExprKind::Binary { op, left, right } => binary_shape(op, left, right),
        ExprKind::Record { path, fields } => {
            if path.len() > 1 {
                return None;
            }
            let type_name = path.first().cloned();
            let shape = RecordShape::from_fields(
                type_name,
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
                    .unwrap_or(ValueShape::Unknown)
                })
                .collect::<Vec<_>>();
            let first = shapes.pop()?;
            if shapes.iter().all(|shape| shape == &first) {
                Some(ValueShape::Array(Box::new(first)))
            } else {
                Some(ValueShape::Array(Box::new(ValueShape::Unknown)))
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
        | BinaryOp::IdentityEqual
        | BinaryOp::IdentityNotEqual
        | BinaryOp::Less
        | BinaryOp::LessEqual
        | BinaryOp::Greater
        | BinaryOp::GreaterEqual
        | BinaryOp::And
        | BinaryOp::Or => Some(ValueShape::Scalar("bool".to_owned())),
        BinaryOp::Range | BinaryOp::RangeInclusive => Some(ValueShape::Scalar("Range".to_owned())),
    }
}

fn arithmetic_shape(left: &Expr, right: &Expr) -> Option<String> {
    let left_float = matches!(left.kind, ExprKind::Literal(Literal::Float(_)));
    let right_float = matches!(right.kind, ExprKind::Literal(Literal::Float(_)));
    Some(if left_float || right_float {
        "f64".to_owned()
    } else {
        "i64".to_owned()
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
    local_type_at_span: &impl Fn(Span) -> Option<RuntimeTypeFact>,
    local_type_named: &impl Fn(&str) -> Option<RuntimeTypeFact>,
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
    local_type_at_span: &impl Fn(Span) -> Option<RuntimeTypeFact>,
    local_type_named: &impl Fn(&str) -> Option<RuntimeTypeFact>,
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
        ("fs", "read_to_string") => Some(ValueShape::Result {
            ok: Some(Box::new(ValueShape::Scalar("String".to_owned()))),
            err: Some(Box::new(ValueShape::Record(
                RecordShape::from_field_shapes([
                    ("kind".to_owned(), ValueShape::Scalar("String".to_owned())),
                    (
                        "message".to_owned(),
                        ValueShape::Scalar("String".to_owned()),
                    ),
                    ("path".to_owned(), ValueShape::Scalar("String".to_owned())),
                ]),
            ))),
        }),
        ("fs", "write_string") | ("io", "print") | ("io", "println") => Some(ValueShape::Result {
            ok: Some(Box::new(ValueShape::Scalar("null".to_owned()))),
            err: Some(Box::new(ValueShape::Record(
                RecordShape::from_field_shapes([
                    ("kind".to_owned(), ValueShape::Scalar("String".to_owned())),
                    (
                        "message".to_owned(),
                        ValueShape::Scalar("String".to_owned()),
                    ),
                    ("path".to_owned(), ValueShape::Scalar("String".to_owned())),
                ]),
            ))),
        }),
        ("option", "some") => Some(ValueShape::Option(Box::new(first??))),
        ("option", "none") => Some(ValueShape::Option(Box::new(ValueShape::Unknown))),
        ("option", "unwrap_or") => {
            let option = first??;
            let fallback = || {
                args.get(1).and_then(|arg| {
                    expression_value_shape(
                        &arg.value,
                        local_shape_at_span,
                        local_shape_named,
                        local_type_at_span,
                        local_type_named,
                    )
                })
            };
            match option {
                ValueShape::Option(value) if !matches!(value.as_ref(), ValueShape::Unknown) => {
                    Some(*value)
                }
                ValueShape::Option(_) => fallback(),
                _ => fallback(),
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
            let fallback = || {
                args.get(1).and_then(|arg| {
                    expression_value_shape(
                        &arg.value,
                        local_shape_at_span,
                        local_shape_named,
                        local_type_at_span,
                        local_type_named,
                    )
                })
            };
            match result {
                ValueShape::Result { ok: Some(ok), .. }
                    if !matches!(ok.as_ref(), ValueShape::Unknown) =>
                {
                    Some(*ok)
                }
                ValueShape::Result { .. } => fallback(),
                _ => fallback(),
            }
        }
        ("set", "from_array") => {
            let array = first??;
            array
                .array_element()
                .cloned()
                .map(|element| ValueShape::Set(Box::new(element)))
        }
        ("reflect", function) => {
            record_reflection_shapes::native_call_shape(function, first.flatten())
        }
        _ => None,
    }
}

fn method_call_shape(
    base: &Expr,
    method: &str,
    args: &[vela_syntax::ast::Argument],
    local_shape_at_span: &impl Fn(Span) -> Option<ValueShape>,
    local_shape_named: &impl Fn(&str) -> Option<ValueShape>,
    local_type_at_span: &impl Fn(Span) -> Option<RuntimeTypeFact>,
    local_type_named: &impl Fn(&str) -> Option<RuntimeTypeFact>,
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
            Some(ValueShape::Scalar("String".to_owned()))
        }
        "join" => Some(ValueShape::Scalar("String".to_owned())),
        "len" | "count" | "sum" => Some(ValueShape::Scalar("i64".to_owned())),
        "has" | "contains" | "starts_with" | "ends_with" | "is_empty" | "is_none" | "is_some"
        | "is_ok" | "is_err" | "any" | "all" | "is_subset" | "is_superset" | "is_disjoint" => {
            Some(ValueShape::Scalar("bool".to_owned()))
        }
        "slice" => match receiver.value_type() {
            Some(RuntimeTypeFact::Primitive(PrimitiveTag::String)) => {
                Some(ValueShape::Scalar("String".to_owned()))
            }
            Some(RuntimeTypeFact::Standard(StandardRuntimeType::Array))
            | Some(RuntimeTypeFact::Array(_)) => Some(receiver),
            _ => None,
        },
        "parse_i64" => Some(ValueShape::Option(Box::new(ValueShape::Scalar(
            "i64".to_owned(),
        )))),
        "parse_f64" => Some(ValueShape::Option(Box::new(ValueShape::Scalar(
            "f64".to_owned(),
        )))),
        "parse_bool" => Some(ValueShape::Option(Box::new(ValueShape::Scalar(
            "bool".to_owned(),
        )))),
        "split" | "split_whitespace" | "split_lines" => Some(ValueShape::Array(Box::new(
            ValueShape::Scalar("String".to_owned()),
        ))),
        "split_once" => Some(ValueShape::Option(Box::new(ValueShape::Array(Box::new(
            ValueShape::Scalar("String".to_owned()),
        ))))),
        "strip_prefix" | "strip_suffix" => Some(ValueShape::Option(Box::new(ValueShape::Scalar(
            "string".to_owned(),
        )))),
        "filter" => match &receiver {
            ValueShape::Array(_) | ValueShape::Map { .. } | ValueShape::Set(_) => Some(receiver),
            ValueShape::Iterator(item) => Some(ValueShape::Iterator(item.clone())),
            ValueShape::Option(value) => Some(ValueShape::Option(value.clone())),
            _ => None,
        },
        "map" => callback_return_shape(&receiver, method, args).map(|value| match receiver {
            ValueShape::Array(_) => ValueShape::Array(Box::new(value)),
            ValueShape::Set(_) => ValueShape::Set(Box::new(value)),
            ValueShape::Iterator(_) => ValueShape::Iterator(Box::new(value)),
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
            ValueShape::Option(value) if !matches!(value.as_ref(), ValueShape::Unknown) => {
                Some((**value).clone())
            }
            ValueShape::Option(_) => args.first().and_then(|arg| {
                expression_value_shape(
                    &arg.value,
                    local_shape_at_span,
                    local_shape_named,
                    local_type_at_span,
                    local_type_named,
                )
            }),
            ValueShape::Result { ok: Some(ok), .. }
                if !matches!(ok.as_ref(), ValueShape::Unknown) =>
            {
                Some((**ok).clone())
            }
            ValueShape::Result { .. } => args.first().and_then(|arg| {
                expression_value_shape(
                    &arg.value,
                    local_shape_at_span,
                    local_shape_named,
                    local_type_at_span,
                    local_type_named,
                )
            }),
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
        "to_option" => match &receiver {
            ValueShape::Result { ok, .. } => Some(ValueShape::Option(
                ok.clone().unwrap_or(Box::new(ValueShape::Unknown)),
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
            ValueShape::Scalar(type_name) if type_name == "String" => Some(ValueShape::Option(
                Box::new(ValueShape::Scalar("i64".to_owned())),
            )),
            _ => None,
        },
        "get" => receiver
            .map_parts()
            .map(|(_, value)| ValueShape::Option(Box::new(value.clone()))),
        "get_or" => receiver.map_parts().map(|(_, value)| value.clone()),
        "index_of" | "last_index_of" => Some(ValueShape::Option(Box::new(ValueShape::Scalar(
            "i64".to_owned(),
        )))),
        "first" | "last" => receiver
            .array_element()
            .cloned()
            .map(|element| ValueShape::Option(Box::new(element))),
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
        "take" | "skip" => receiver
            .iterator_item()
            .cloned()
            .map(|item| ValueShape::Iterator(Box::new(item))),
        "collect_array" => receiver
            .iterator_item()
            .cloned()
            .map(|item| ValueShape::Array(Box::new(item))),
        "group_by" => receiver
            .array_element()
            .cloned()
            .map(|element| ValueShape::Map {
                key: Box::new(ValueShape::Scalar("String".to_owned())),
                value: Box::new(ValueShape::Array(Box::new(element))),
            }),
        "sort" | "sort_by" | "reverse" | "distinct" => Some(receiver),
        "keys" => receiver
            .map_parts()
            .map(|(key, _)| ValueShape::Iterator(Box::new(key.clone()))),
        "values" => match &receiver {
            ValueShape::Array(value) | ValueShape::Set(value) | ValueShape::Map { value, .. } => {
                Some(ValueShape::Iterator(value.clone()))
            }
            _ => None,
        },
        "entries" => receiver.map_parts().map(|(key, value)| {
            ValueShape::Iterator(Box::new(ValueShape::Record(
                RecordShape::from_field_shapes([
                    ("key".to_owned(), key.clone()),
                    ("value".to_owned(), value.clone()),
                ]),
            )))
        }),
        _ => None,
    }
}

pub(super) fn callback_return_shape(
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
    pub(in crate::compiler) fn value_shape_for_expr_with_payload(
        &self,
        expr: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> Option<ValueShape> {
        if let Some(shape) =
            payload.and_then(|payload| self.value_shape_for_syntax_payload(payload))
        {
            return Some(shape);
        }
        match &expr.kind {
            ExprKind::Path(path) => self.value_shape_for_path_expr(expr.span, path, payload),
            ExprKind::Field { base, name } => {
                let field_name = payload
                    .and_then(CompilerExpressionPayload::field_name)
                    .unwrap_or_else(|| name.clone());
                let base_payload = payload.and_then(CompilerExpressionPayload::field_base_payload);
                self.value_shape_for_expr_with_payload(base, base_payload.as_ref())?
                    .as_record()?
                    .field_value_shape(&field_name)
                    .cloned()
            }
            _ => self.value_shape_for_expr_legacy(expr),
        }
    }

    fn value_shape_for_expr_legacy(&self, expr: &Expr) -> Option<ValueShape> {
        expression_value_shape(
            expr,
            &|span| {
                self.value_shapes
                    .local_at_span(self.bindings, span)
                    .or_else(|| {
                        self.script_types
                            .local_at_span(self.bindings, span)
                            .and_then(|type_name| self.record_shape_for_type(&type_name))
                            .map(ValueShape::Record)
                    })
            },
            &|name| {
                self.value_shapes.name(name).or_else(|| {
                    self.script_types
                        .name(name)
                        .or_else(|| self.global_type_named(name))
                        .and_then(|type_name| self.record_shape_for_type(&type_name))
                        .map(ValueShape::Record)
                })
            },
            &|span| self.value_types.local_at_span(self.bindings, span),
            &|name| self.value_types.name(name),
        )
    }

    pub(in crate::compiler) fn record_shape_for_expr_with_payload(
        &self,
        expr: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> Option<RecordShape> {
        self.value_shape_for_expr_with_payload(expr, payload)?
            .as_record()
            .cloned()
    }

    pub(super) fn record_shape_for_path_root(&self, span: Span, root: &str) -> Option<RecordShape> {
        self.value_shapes
            .local_at_span(self.bindings, span)
            .or_else(|| self.value_shapes.name(root))
            .and_then(|shape| shape.as_record().cloned())
            .or_else(|| {
                self.global_type_at_span(span)
                    .or_else(|| self.global_type_named(root))
                    .and_then(|type_name| self.record_shape_for_type(&type_name))
            })
    }

    pub(super) fn record_shape_for_index_collection(
        &self,
        collection: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> Option<RecordShape> {
        self.value_shape_for_expr_with_payload(collection, payload)?
            .array_element_record()
            .cloned()
    }

    pub(in crate::compiler) fn record_field_value_type_for_expr_with_payload(
        &self,
        expr: &Expr,
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> Option<RuntimeTypeFact> {
        let ExprKind::Field { base, name } = &expr.kind else {
            return None;
        };
        let field_name = payload
            .and_then(CompilerExpressionPayload::field_name)
            .unwrap_or_else(|| name.clone());
        let base_payload = payload.and_then(CompilerExpressionPayload::field_base_payload);
        self.record_shape_for_expr_with_payload(base, base_payload.as_ref())?
            .field_value_type(&field_name)
    }

    fn value_shape_for_path_expr(
        &self,
        span: Span,
        legacy_path: &[String],
        payload: Option<&CompilerExpressionPayload<'_>>,
    ) -> Option<ValueShape> {
        let local_shape = self.value_shapes.local_at_span(self.bindings, span);
        let cst_path = payload.and_then(CompilerExpressionPayload::path_segments);
        if let Some([root]) = cst_path.as_deref() {
            let cst_shape = self.value_shapes.name(root).or_else(|| {
                self.script_types
                    .name(root)
                    .or_else(|| self.global_type_named(root))
                    .and_then(|type_name| self.record_shape_for_type(&type_name))
                    .map(ValueShape::Record)
            });
            return cst_shape.or(local_shape);
        }
        let path = cst_path.as_deref().unwrap_or(legacy_path);
        let [root] = path else {
            return local_shape;
        };
        local_shape
            .or_else(|| self.value_shapes.name(root))
            .or_else(|| {
                self.script_types
                    .name(root)
                    .or_else(|| self.global_type_named(root))
                    .and_then(|type_name| self.record_shape_for_type(&type_name))
                    .map(ValueShape::Record)
            })
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
        self.value_shapes.name(receiver).or_else(|| {
            self.global_type_named(receiver)
                .and_then(|type_name| self.record_shape_for_type(&type_name))
                .map(ValueShape::Record)
        })
    }

    pub(super) fn record_shape_for_type(&self, type_name: &str) -> Option<RecordShape> {
        self.record_shape_for_type_inner(type_name, &mut BTreeSet::new())
    }

    fn record_shape_for_type_inner(
        &self,
        type_name: &str,
        visiting: &mut BTreeSet<String>,
    ) -> Option<RecordShape> {
        if !visiting.insert(type_name.to_owned()) {
            return None;
        }
        let fields = self
            .facts
            .script_field_slots
            .record_fields(type_name)
            .into_iter()
            .map(|(field, script_fact, value_type)| {
                let value = script_fact
                    .as_ref()
                    .and_then(|fact| {
                        self.record_shape_for_type_inner(&fact.type_name, visiting)
                            .map(ValueShape::Record)
                    })
                    .or_else(|| value_type.map(ValueShape::from_runtime_type))
                    .unwrap_or(ValueShape::Unknown);
                (field, value)
            })
            .collect::<Vec<_>>();
        visiting.remove(type_name);
        (!fields.is_empty())
            .then(|| RecordShape::from_field_shapes_with_type(Some(type_name.to_owned()), fields))
    }
}
