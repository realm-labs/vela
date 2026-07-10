use std::collections::BTreeMap;

use vela_common::PrimitiveTag;
use vela_hir::body::{HirBinaryOp, HirExprKind, HirLiteral};
use vela_hir::ids::HirExprId;

use crate::compiler::Compiler;

use super::{RecordFieldShape, RecordShape, ValueShape};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn hir_method_result_shape(
        &self,
        receiver: ValueShape,
        method: &str,
        first: Option<ValueShape>,
        callback: Option<ValueShape>,
    ) -> Option<ValueShape> {
        hir_method_shape(receiver, method, first, callback)
    }

    pub(in crate::compiler) fn value_shape_for_hir_expression(
        &self,
        expression: HirExprId,
    ) -> Option<ValueShape> {
        let kind = self
            .hir_bodies
            .iter()
            .find_map(|body| body.expression(expression))?
            .kind
            .clone();
        match kind {
            HirExprKind::Literal(literal) => hir_literal_shape(&literal),
            HirExprKind::Unit => Some(ValueShape::Scalar("()".to_owned())),
            HirExprKind::Array { elements } => {
                let shapes = elements
                    .into_iter()
                    .map(|value| {
                        self.value_shape_for_hir_expression(value)
                            .unwrap_or(ValueShape::Unknown)
                    })
                    .collect::<Vec<_>>();
                Some(ValueShape::Array(Box::new(common_hir_shape(shapes))))
            }
            HirExprKind::Tuple { elements } => Some(ValueShape::Tuple(
                elements
                    .into_iter()
                    .map(|value| {
                        self.value_shape_for_hir_expression(value)
                            .unwrap_or(ValueShape::Unknown)
                    })
                    .collect(),
            )),
            HirExprKind::Map { entries } => {
                let keys = entries
                    .iter()
                    .filter_map(|entry| entry.key)
                    .map(|value| {
                        self.value_shape_for_hir_expression(value)
                            .unwrap_or(ValueShape::Unknown)
                    })
                    .collect::<Vec<_>>();
                let values = entries
                    .iter()
                    .filter_map(|entry| entry.value)
                    .map(|value| {
                        self.value_shape_for_hir_expression(value)
                            .unwrap_or(ValueShape::Unknown)
                    })
                    .collect::<Vec<_>>();
                Some(ValueShape::Map {
                    key: Box::new(common_hir_shape(keys)),
                    value: Box::new(common_hir_shape(values)),
                })
            }
            HirExprKind::Record { fields, .. } => {
                let path = self.hir_constructor_path(expression)?;
                if crate::compiler::patterns::enum_variant_path(path).is_some() {
                    return None;
                }
                let type_name = self
                    .type_symbol_for_expression(expression)
                    .or_else(|| (!path.is_empty()).then(|| path.join("::")));
                let mut names = fields
                    .iter()
                    .map(|field| field.name.clone())
                    .collect::<Vec<_>>();
                names.sort_unstable();
                names.dedup();
                if names.is_empty() {
                    return type_name
                        .as_deref()
                        .and_then(|name| self.record_shape_for_type(name))
                        .map(ValueShape::Record);
                }
                let slots = names
                    .into_iter()
                    .enumerate()
                    .map(|(slot, name)| (name, slot))
                    .collect::<BTreeMap<_, _>>();
                let fields = fields
                    .into_iter()
                    .filter_map(|field| {
                        let slot = slots.get(&field.name).copied()?;
                        let value = field
                            .value
                            .and_then(|value| self.value_shape_for_hir_expression(value));
                        let value_type = value.as_ref().and_then(ValueShape::value_type);
                        Some((
                            field.name,
                            RecordFieldShape {
                                slot,
                                value_type,
                                value,
                            },
                        ))
                    })
                    .collect();
                Some(ValueShape::Record(RecordShape { type_name, fields }))
            }
            HirExprKind::Path(_) => self
                .local_for_expression(expression)
                .and_then(|local| self.value_shapes.local(local)),
            HirExprKind::Field(field) => self
                .value_shape_for_hir_expression(field.receiver)?
                .as_record()?
                .field_value_shape(&field.name)
                .cloned(),
            HirExprKind::Index(index) => {
                match self.value_shape_for_hir_expression(index.receiver)? {
                    ValueShape::Array(element) => Some(*element),
                    ValueShape::Map { value, .. } => Some(*value),
                    _ => None,
                }
            }
            HirExprKind::Paren {
                expression: Some(inner),
            } => self.value_shape_for_hir_expression(inner),
            HirExprKind::Binary { op: Some(op), .. } => Some(match op {
                HirBinaryOp::Equal
                | HirBinaryOp::NotEqual
                | HirBinaryOp::IdentityEqual
                | HirBinaryOp::IdentityNotEqual
                | HirBinaryOp::Less
                | HirBinaryOp::LessEqual
                | HirBinaryOp::Greater
                | HirBinaryOp::GreaterEqual
                | HirBinaryOp::And
                | HirBinaryOp::Or => ValueShape::Scalar("bool".to_owned()),
                HirBinaryOp::Range | HirBinaryOp::RangeInclusive => {
                    ValueShape::Scalar("Range".to_owned())
                }
                HirBinaryOp::Add
                | HirBinaryOp::Sub
                | HirBinaryOp::Mul
                | HirBinaryOp::Div
                | HirBinaryOp::Rem => ValueShape::Unknown,
            }),
            HirExprKind::Try {
                expression: Some(inner),
            } => match self.value_shape_for_hir_expression(inner)? {
                ValueShape::Option(value) => Some(*value),
                ValueShape::Result { ok: Some(ok), .. } => Some(*ok),
                ValueShape::Result { .. } => Some(ValueShape::Unknown),
                _ => None,
            },
            HirExprKind::Call(call) => self.hir_call_shape(&call),
            HirExprKind::Paren { expression: None }
            | HirExprKind::Unary { .. }
            | HirExprKind::Binary { .. }
            | HirExprKind::Assign { .. }
            | HirExprKind::Try { expression: None }
            | HirExprKind::Lambda { .. }
            | HirExprKind::Block { .. }
            | HirExprKind::If(_)
            | HirExprKind::Match(_)
            | HirExprKind::Missing => None,
        }
    }

    fn hir_call_shape(&self, call: &vela_hir::body::HirCall) -> Option<ValueShape> {
        if let Some(field) = self.hir_field_for_expression(call.callee) {
            let receiver = self.value_shape_for_hir_expression(field.receiver)?;
            let first = call
                .arguments
                .first()
                .and_then(|argument| argument.value)
                .and_then(|value| self.value_shape_for_hir_expression(value));
            let callback = call
                .arguments
                .first()
                .and_then(|argument| argument.value)
                .and_then(|value| {
                    self.hir_callback_return_shape(Some(&receiver), &field.name, value)
                });
            return hir_method_shape(receiver, &field.name, first, callback);
        }
        let path = self.hir_callee_path(call.expression)?;
        let [module, function] = path else {
            return None;
        };
        let first = call
            .arguments
            .first()
            .and_then(|argument| argument.value)
            .and_then(|value| self.value_shape_for_hir_expression(value));
        match (module.as_str(), function.as_str()) {
            ("option", "some") => Some(ValueShape::Option(Box::new(first?))),
            ("option", "none") => Some(ValueShape::Option(Box::new(ValueShape::Unknown))),
            ("result", "ok") => Some(ValueShape::Result {
                ok: Some(Box::new(first?)),
                err: None,
            }),
            ("result", "err") => Some(ValueShape::Result {
                ok: None,
                err: Some(Box::new(first?)),
            }),
            ("set", "from_array") => first?
                .array_element()
                .cloned()
                .map(|element| ValueShape::Set(Box::new(element))),
            ("reflect", function) => {
                crate::compiler::record_reflection_shapes::native_call_shape(function, first)
            }
            _ => None,
        }
    }
}

fn hir_method_shape(
    receiver: ValueShape,
    method: &str,
    first: Option<ValueShape>,
    callback: Option<ValueShape>,
) -> Option<ValueShape> {
    match method {
        "to_upper" | "to_lower" | "trim" | "trim_start" | "trim_end" | "replace" | "repeat"
        | "join" => Some(string_shape()),
        "len" | "count" | "sum" => Some(i64_shape()),
        "has" | "contains" | "starts_with" | "ends_with" | "is_empty" | "is_none" | "is_some"
        | "is_ok" | "is_err" | "any" | "all" | "is_subset" | "is_superset" | "is_disjoint" => {
            Some(bool_shape())
        }
        "slice" => Some(receiver),
        "split" | "split_whitespace" | "split_lines" => {
            Some(ValueShape::Array(Box::new(string_shape())))
        }
        "split_once" => Some(ValueShape::Option(Box::new(ValueShape::Tuple(vec![
            string_shape(),
            string_shape(),
        ])))),
        "strip_prefix" | "strip_suffix" => Some(ValueShape::Option(Box::new(string_shape()))),
        "find" => match receiver {
            ValueShape::Scalar(ref name) if name == "String" => {
                Some(ValueShape::Option(Box::new(i64_shape())))
            }
            ValueShape::Array(value) | ValueShape::Set(value) | ValueShape::Iterator(value) => {
                Some(ValueShape::Option(value))
            }
            _ => None,
        },
        "chars" => Some(ValueShape::Iterator(Box::new(ValueShape::Scalar(
            "char".to_owned(),
        )))),
        "bytes" => Some(ValueShape::Iterator(Box::new(ValueShape::Scalar(
            "u8".to_owned(),
        )))),
        "first" | "last" | "pop" | "remove_at" | "min" | "max" => match receiver {
            ValueShape::Array(value) => Some(ValueShape::Option(value)),
            _ => None,
        },
        "get" => match receiver {
            ValueShape::Map { value, .. } => Some(ValueShape::Option(value)),
            _ => None,
        },
        "get_or" => match receiver {
            ValueShape::Map { value, .. } => Some(*value),
            _ => None,
        },
        "values" => match receiver {
            ValueShape::Array(value) | ValueShape::Set(value) | ValueShape::Map { value, .. } => {
                Some(ValueShape::Iterator(value))
            }
            _ => None,
        },
        "collect_array" => match receiver {
            ValueShape::Iterator(value) => Some(ValueShape::Array(value)),
            _ => None,
        },
        "sort" | "sort_by" | "reverse" | "distinct" | "filter" => Some(receiver),
        "union" | "intersection" | "difference" | "symmetric_difference" => match receiver {
            ValueShape::Set(_) => Some(receiver),
            _ => None,
        },
        "map" => match receiver {
            ValueShape::Array(_) => Some(ValueShape::Array(Box::new(
                callback.unwrap_or(ValueShape::Unknown),
            ))),
            ValueShape::Set(_) => Some(ValueShape::Set(Box::new(
                callback.unwrap_or(ValueShape::Unknown),
            ))),
            ValueShape::Iterator(_) => Some(ValueShape::Iterator(Box::new(
                callback.unwrap_or(ValueShape::Unknown),
            ))),
            ValueShape::Option(_) => Some(ValueShape::Option(Box::new(
                callback.unwrap_or(ValueShape::Unknown),
            ))),
            ValueShape::Result { err, .. } => Some(ValueShape::Result {
                ok: Some(Box::new(callback.unwrap_or(ValueShape::Unknown))),
                err,
            }),
            _ => callback,
        },
        "map_err" => match receiver {
            ValueShape::Result { ok, .. } => Some(ValueShape::Result {
                ok,
                err: Some(Box::new(callback.unwrap_or(ValueShape::Unknown))),
            }),
            _ => callback,
        },
        "group_by" => match receiver {
            ValueShape::Array(element) => Some(ValueShape::Map {
                key: Box::new(callback.unwrap_or(ValueShape::Unknown)),
                value: Box::new(ValueShape::Array(element)),
            }),
            _ => None,
        },
        "and_then" | "or_else" => callback,
        "unwrap_or" => match receiver {
            ValueShape::Option(value) => Some(if matches!(value.as_ref(), ValueShape::Unknown) {
                first.unwrap_or(*value)
            } else {
                *value
            }),
            ValueShape::Result { ok: Some(ok), .. } => {
                Some(if matches!(ok.as_ref(), ValueShape::Unknown) {
                    first.unwrap_or(*ok)
                } else {
                    *ok
                })
            }
            ValueShape::Result { .. } => first,
            _ => None,
        },
        "ok_or" => match receiver {
            ValueShape::Option(value) => Some(ValueShape::Result {
                ok: Some(value),
                err: first.map(Box::new),
            }),
            _ => None,
        },
        "to_option" => match receiver {
            ValueShape::Result { ok, .. } => Some(ValueShape::Option(
                ok.unwrap_or(Box::new(ValueShape::Unknown)),
            )),
            _ => None,
        },
        "to_error_option" => match receiver {
            ValueShape::Result { err, .. } => Some(ValueShape::Option(
                err.unwrap_or(Box::new(ValueShape::Unknown)),
            )),
            _ => None,
        },
        "flatten" => match receiver {
            ValueShape::Option(value) => match *value {
                ValueShape::Option(inner) => Some(ValueShape::Option(inner)),
                value => Some(ValueShape::Option(Box::new(value))),
            },
            ValueShape::Result { ok, err } => match ok.map(|ok| *ok) {
                Some(ValueShape::Result { ok, err: inner }) => Some(ValueShape::Result {
                    ok,
                    err: inner.or(err),
                }),
                ok => Some(ValueShape::Result {
                    ok: ok.map(Box::new),
                    err,
                }),
            },
            _ => None,
        },
        _ => None,
    }
}

fn string_shape() -> ValueShape {
    ValueShape::Scalar("String".to_owned())
}

fn i64_shape() -> ValueShape {
    ValueShape::Scalar("i64".to_owned())
}

fn bool_shape() -> ValueShape {
    ValueShape::Scalar("bool".to_owned())
}

fn common_hir_shape(shapes: Vec<ValueShape>) -> ValueShape {
    let Some(first) = shapes.first() else {
        return ValueShape::Unknown;
    };
    if shapes.iter().all(|shape| shape == first) {
        first.clone()
    } else {
        ValueShape::Unknown
    }
}

fn hir_literal_shape(literal: &HirLiteral) -> Option<ValueShape> {
    let tag = match literal {
        HirLiteral::Bool(_) => PrimitiveTag::Bool,
        HirLiteral::Char(_) => PrimitiveTag::Char,
        HirLiteral::Integer(value) => super::super::hir_lowering::integer_suffix_tag(value.suffix),
        HirLiteral::Float(value) => super::super::hir_lowering::float_suffix_tag(value.suffix),
        HirLiteral::String(_) | HirLiteral::Interpolated { .. } => PrimitiveTag::String,
        HirLiteral::Bytes(_) => PrimitiveTag::Bytes,
        HirLiteral::Invalid { .. } => return None,
    };
    Some(ValueShape::from_runtime_type(
        super::super::value_types::RuntimeTypeFact::primitive(tag),
    ))
}
