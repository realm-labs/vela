use std::collections::BTreeMap;

use vela_common::{PrimitiveTag, SourceId, Span};
use vela_syntax::SyntaxKind;
use vela_syntax::ast::{
    AstNode, BinaryOp, Literal, SyntaxArgument, SyntaxExpression, SyntaxExpressionKind,
};

use crate::compiler::Compiler;
use crate::compiler::value_types::{RuntimeTypeFact, StandardRuntimeType};
use vela_hir::ids::HirExprId;

use super::{RecordFieldShape, RecordShape, ValueShape, common_shape, record_reflection_shapes};

impl Compiler<'_, '_> {
    pub(in crate::compiler) fn record_shape_for_syntax_expression(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<RecordShape> {
        self.value_shape_for_syntax_expression(source, expression)?
            .as_record()
            .cloned()
    }

    pub(in crate::compiler) fn value_shape_for_syntax_expression(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<ValueShape> {
        match expression.expression_kind() {
            SyntaxExpressionKind::Literal => self.literal_shape(expression),
            SyntaxExpressionKind::Array => self.array_shape(source, expression),
            SyntaxExpressionKind::Tuple => self.tuple_shape(source, expression),
            SyntaxExpressionKind::Map => self.map_shape(source, expression),
            SyntaxExpressionKind::Record => self.record_shape(source, expression),
            SyntaxExpressionKind::Path => self.path_shape(source, expression),
            SyntaxExpressionKind::Field => self.field_shape(source, expression),
            SyntaxExpressionKind::Call => self.call_shape(source, expression),
            SyntaxExpressionKind::Index => self.index_shape(source, expression),
            SyntaxExpressionKind::Paren => self.paren_shape(source, expression),
            SyntaxExpressionKind::Binary => self.binary_shape(expression),
            SyntaxExpressionKind::Try => self.try_shape(source, expression),
            SyntaxExpressionKind::Unit
            | SyntaxExpressionKind::Unary
            | SyntaxExpressionKind::Assign
            | SyntaxExpressionKind::Lambda
            | SyntaxExpressionKind::Block
            | SyntaxExpressionKind::If
            | SyntaxExpressionKind::Match => None,
        }
    }

    fn literal_shape(&self, expression: &SyntaxExpression) -> Option<ValueShape> {
        let literal = expression.as_literal()?;
        if literal.token_kind() == Some(SyntaxKind::InterpolatedString) {
            return Some(ValueShape::Scalar("String".to_owned()));
        }
        literal
            .literal()
            .map(literal_type)
            .map(ValueShape::from_runtime_type)
    }

    fn array_shape(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<ValueShape> {
        let values = expression.as_array()?.expressions().collect::<Vec<_>>();
        if values.is_empty() {
            return Some(ValueShape::Array(Box::new(ValueShape::Unknown)));
        }
        let mut shapes = values
            .iter()
            .map(|value| {
                self.value_shape_for_syntax_expression(source, value)
                    .unwrap_or(ValueShape::Unknown)
            })
            .collect::<Vec<_>>();
        let first = shapes.pop()?;
        let element = if shapes.iter().all(|shape| shape == &first) {
            first
        } else {
            ValueShape::Unknown
        };
        Some(ValueShape::Array(Box::new(element)))
    }

    fn tuple_shape(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<ValueShape> {
        let elements = expression
            .as_tuple()?
            .expressions()
            .map(|value| {
                self.value_shape_for_syntax_expression(source, &value)
                    .unwrap_or(ValueShape::Unknown)
            })
            .collect::<Vec<_>>();
        Some(ValueShape::Tuple(elements))
    }

    fn map_shape(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<ValueShape> {
        let entries = expression.as_map()?.entries().collect::<Vec<_>>();
        if entries.is_empty() {
            return Some(ValueShape::Map {
                key: Box::new(ValueShape::Unknown),
                value: Box::new(ValueShape::Unknown),
            });
        }

        let mut keys = entries
            .iter()
            .map(|entry| {
                entry
                    .key()
                    .and_then(|key| self.value_shape_for_syntax_expression(source, &key))
            })
            .collect::<Option<Vec<_>>>()?;
        let key = keys.pop()?;
        if !keys.iter().all(|shape| shape == &key) {
            return None;
        }

        let values = entries
            .iter()
            .map(|entry| {
                entry
                    .value()
                    .and_then(|value| self.value_shape_for_syntax_expression(source, &value))
            })
            .collect::<Option<Vec<_>>>();
        let value = values.and_then(common_shape).unwrap_or(ValueShape::Unknown);
        Some(ValueShape::Map {
            key: Box::new(key),
            value: Box::new(value),
        })
    }

    fn record_shape(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<ValueShape> {
        let record = expression.as_record()?;
        if self.is_enum_variant_record_literal(source, expression) {
            return None;
        }
        let type_name = self.record_literal_type_name(source, expression);
        let fields = record.fields();
        let mut field_names = fields
            .iter()
            .filter_map(|field| field.label_text())
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
            .into_iter()
            .filter_map(|field| {
                let name = field.label_text()?;
                let slot = slots.get(&name).copied()?;
                let value = field
                    .expression()
                    .and_then(|value| self.value_shape_for_syntax_expression(source, &value));
                let value_type = field
                    .expression()
                    .and_then(|value| self.value_type_for_syntax_expression(source, &value));
                Some((
                    name,
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

    fn path_shape(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<ValueShape> {
        let source = source?;
        let span = syntax_expression_span(source, expression);
        let path = self.hir_value_path_for_span(span)?;
        self.value_shape_for_path(span, &path)
    }

    fn field_shape(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<ValueShape> {
        let source = source?;
        let span = syntax_expression_span(source, expression);
        let field = self.hir_field_for_span(span)?;
        let receiver_span = self.expression_span(field.receiver)?;
        let receiver = syntax_shape_expression_at_span(source, expression, receiver_span)?;
        self.value_shape_for_syntax_expression(Some(source), &receiver)?
            .as_record()?
            .field_value_shape(&field.name)
            .cloned()
    }

    fn paren_shape(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<ValueShape> {
        let inner = expression.as_paren()?.expression()?;
        self.value_shape_for_syntax_expression(source, &inner)
    }

    fn try_shape(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<ValueShape> {
        let operand = expression.as_try()?.expression()?;
        unwrap_try_shape(self.value_shape_for_syntax_expression(source, &operand)?)
    }

    fn binary_shape(&self, expression: &SyntaxExpression) -> Option<ValueShape> {
        let binary = expression.as_binary()?;
        match binary.operator()? {
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
            BinaryOp::Range | BinaryOp::RangeInclusive => {
                Some(ValueShape::Scalar("Range".to_owned()))
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                Some(ValueShape::Scalar(arithmetic_shape(&binary)?))
            }
        }
    }

    fn index_shape(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<ValueShape> {
        let source = source?;
        let span = syntax_expression_span(source, expression);
        let index = self.hir_index_for_span(span)?;
        let receiver_span = self.expression_span(index.receiver)?;
        let receiver = syntax_shape_expression_at_span(source, expression, receiver_span)?;
        match self.value_shape_for_syntax_expression(Some(source), &receiver)? {
            ValueShape::Array(element) => Some(*element),
            ValueShape::Map { value, .. } => Some(*value),
            _ => None,
        }
    }

    fn call_shape(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<ValueShape> {
        let call = expression.as_call()?;
        let args = call.arguments();
        let source = source?;
        let call_expression =
            self.expression_at_span(syntax_expression_span(source, expression))?;
        if let Some(path) = self.hir_callee_path(call_expression) {
            return self.native_call_shape(Some(source), path, &args);
        }
        let callee = self.call_callee_expression(call_expression)?;
        self.method_call_shape(Some(source), expression, callee, &args)
    }

    fn native_call_shape(
        &self,
        source: Option<SourceId>,
        path: &[String],
        args: &[SyntaxArgument],
    ) -> Option<ValueShape> {
        let [module, function] = path else {
            return None;
        };
        let first = args
            .first()
            .and_then(|arg| arg.expression())
            .and_then(|arg| self.value_shape_for_syntax_expression(source, &arg));
        match (module.as_str(), function.as_str()) {
            ("fs", "read_to_string") => Some(ValueShape::Result {
                ok: Some(Box::new(string_shape())),
                err: Some(Box::new(io_error_shape())),
            }),
            ("fs", "write_string") | ("io", "print") | ("io", "println") => {
                Some(ValueShape::Result {
                    ok: Some(Box::new(ValueShape::Scalar("()".to_owned()))),
                    err: Some(Box::new(io_error_shape())),
                })
            }
            ("option", "some") => Some(ValueShape::Option(Box::new(first?))),
            ("option", "none") => Some(ValueShape::Option(Box::new(ValueShape::Unknown))),
            ("option", "unwrap_or") => match first? {
                ValueShape::Option(value) if !matches!(value.as_ref(), ValueShape::Unknown) => {
                    Some(*value)
                }
                _ => args.get(1).and_then(|arg| {
                    arg.expression()
                        .and_then(|arg| self.value_shape_for_syntax_expression(source, &arg))
                }),
            },
            ("result", "ok") => Some(ValueShape::Result {
                ok: Some(Box::new(first?)),
                err: None,
            }),
            ("result", "err") => Some(ValueShape::Result {
                ok: None,
                err: Some(Box::new(first?)),
            }),
            ("result", "unwrap_or") => match first? {
                ValueShape::Result { ok: Some(ok), .. }
                    if !matches!(ok.as_ref(), ValueShape::Unknown) =>
                {
                    Some(*ok)
                }
                _ => args.get(1).and_then(|arg| {
                    arg.expression()
                        .and_then(|arg| self.value_shape_for_syntax_expression(source, &arg))
                }),
            },
            ("set", "from_array") => first?
                .array_element()
                .cloned()
                .map(|element| ValueShape::Set(Box::new(element))),
            ("reflect", function) => record_reflection_shapes::native_call_shape(function, first),
            _ => None,
        }
    }

    fn method_call_shape(
        &self,
        source: Option<SourceId>,
        call_expression: &SyntaxExpression,
        callee: HirExprId,
        args: &[SyntaxArgument],
    ) -> Option<ValueShape> {
        let source_id = source?;
        let field = self.hir_field_for_expression(callee)?;
        let receiver_span = self.expression_span(field.receiver)?;
        let receiver_expression =
            syntax_shape_expression_at_span(source_id, call_expression, receiver_span)?;
        let source = Some(source_id);
        let receiver = self.value_shape_for_syntax_expression(source, &receiver_expression)?;
        let method = field.name.as_str();
        match method {
            "to_upper" | "to_lower" | "trim" | "trim_start" | "trim_end" | "replace" | "repeat"
            | "join" => Some(string_shape()),
            "len" | "count" | "sum" => Some(ValueShape::Scalar("i64".to_owned())),
            "has" | "contains" | "starts_with" | "ends_with" | "is_empty" | "is_none"
            | "is_some" | "is_ok" | "is_err" | "any" | "all" | "is_subset" | "is_superset"
            | "is_disjoint" => Some(ValueShape::Scalar("bool".to_owned())),
            "slice" => match receiver.value_type() {
                Some(RuntimeTypeFact::Primitive(PrimitiveTag::String)) => Some(string_shape()),
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
            "split" | "split_whitespace" | "split_lines" => {
                Some(ValueShape::Array(Box::new(string_shape())))
            }
            "split_once" => Some(ValueShape::Option(Box::new(ValueShape::Tuple(vec![
                string_shape(),
                string_shape(),
            ])))),
            "strip_prefix" | "strip_suffix" => Some(ValueShape::Option(Box::new(string_shape()))),
            "filter" => match receiver {
                ValueShape::Array(_)
                | ValueShape::Map { .. }
                | ValueShape::Set(_)
                | ValueShape::Option(_) => Some(receiver),
                ValueShape::Iterator(item) => Some(ValueShape::Iterator(item)),
                _ => None,
            },
            "map" => {
                let value = self.syntax_callback_return_shape(&receiver, "map", args, source);
                Some(match receiver {
                    ValueShape::Array(_) => {
                        ValueShape::Array(Box::new(value.unwrap_or(ValueShape::Unknown)))
                    }
                    ValueShape::Set(_) => {
                        ValueShape::Set(Box::new(value.unwrap_or(ValueShape::Unknown)))
                    }
                    ValueShape::Iterator(_) => {
                        ValueShape::Iterator(Box::new(value.unwrap_or(ValueShape::Unknown)))
                    }
                    ValueShape::Option(_) => {
                        ValueShape::Option(Box::new(value.unwrap_or(ValueShape::Unknown)))
                    }
                    ValueShape::Result { err, .. } => ValueShape::Result {
                        ok: Some(Box::new(value.unwrap_or(ValueShape::Unknown))),
                        err,
                    },
                    _ => value?,
                })
            }
            "map_err" => {
                let value =
                    self.syntax_callback_return_shape(&receiver, "map_err", args, source)?;
                Some(match receiver {
                    ValueShape::Result { ok, .. } => ValueShape::Result {
                        ok,
                        err: Some(Box::new(value)),
                    },
                    _ => value,
                })
            }
            "and_then" => self.syntax_callback_return_shape(&receiver, "and_then", args, source),
            "group_by" => {
                let key = self
                    .syntax_callback_return_shape(&receiver, "group_by", args, source)
                    .unwrap_or(ValueShape::Unknown);
                let ValueShape::Array(element) = receiver else {
                    return None;
                };
                Some(ValueShape::Map {
                    key: Box::new(key),
                    value: Box::new(ValueShape::Array(element)),
                })
            }
            "map_values" => {
                let value =
                    self.syntax_callback_return_shape(&receiver, "map_values", args, source)?;
                let (key, _) = receiver.map_parts()?;
                Some(ValueShape::Map {
                    key: Box::new(key.clone()),
                    value: Box::new(value),
                })
            }
            "first" | "last" | "pop" | "remove_at" | "min" | "max" => receiver
                .array_element()
                .cloned()
                .map(|element| ValueShape::Option(Box::new(element))),
            "get" => receiver
                .map_parts()
                .map(|(_, value)| ValueShape::Option(Box::new(value.clone()))),
            "get_or" => receiver.map_parts().map(|(_, value)| value.clone()),
            "find" => match &receiver {
                ValueShape::Scalar(type_name) if type_name == "String" => Some(ValueShape::Option(
                    Box::new(ValueShape::Scalar("i64".to_owned())),
                )),
                ValueShape::Array(value) | ValueShape::Set(value) | ValueShape::Iterator(value) => {
                    Some(ValueShape::Option(value.clone()))
                }
                _ => None,
            },
            "index_of" | "last_index_of" => Some(ValueShape::Option(Box::new(ValueShape::Scalar(
                "i64".to_owned(),
            )))),
            "merge" | "union" | "intersection" | "difference" | "symmetric_difference" => {
                Some(receiver)
            }
            "take" | "skip" => receiver
                .iterator_item()
                .cloned()
                .map(|item| ValueShape::Iterator(Box::new(item))),
            "collect_array" => receiver
                .iterator_item()
                .cloned()
                .map(|item| ValueShape::Array(Box::new(item))),
            "sort" | "sort_by" | "reverse" | "distinct" => Some(receiver),
            "keys" => receiver
                .map_parts()
                .map(|(key, _)| ValueShape::Iterator(Box::new(key.clone()))),
            "values" => match &receiver {
                ValueShape::Array(value)
                | ValueShape::Set(value)
                | ValueShape::Map { value, .. } => Some(ValueShape::Iterator(value.clone())),
                _ => None,
            },
            "entries" => receiver.map_parts().map(|(key, value)| {
                ValueShape::Iterator(Box::new(ValueShape::map_entry(key.clone(), value.clone())))
            }),
            "unwrap_or" => match receiver {
                ValueShape::Option(value) if !matches!(value.as_ref(), ValueShape::Unknown) => {
                    Some(*value)
                }
                ValueShape::Option(_) => args
                    .first()
                    .and_then(|arg| arg.expression())
                    .and_then(|arg| self.value_shape_for_syntax_expression(source, &arg)),
                ValueShape::Result { ok: Some(ok), .. }
                    if !matches!(ok.as_ref(), ValueShape::Unknown) =>
                {
                    Some(*ok)
                }
                ValueShape::Result { .. } => args
                    .first()
                    .and_then(|arg| arg.expression())
                    .and_then(|arg| self.value_shape_for_syntax_expression(source, &arg)),
                _ => None,
            },
            "or_else" => self.syntax_callback_return_shape(&receiver, "or_else", args, source),
            "ok_or" => match receiver {
                ValueShape::Option(value) => Some(ValueShape::Result {
                    ok: Some(value),
                    err: args.first().map(|arg| {
                        Box::new(
                            arg.expression()
                                .and_then(|arg| {
                                    self.value_shape_for_syntax_expression(source, &arg)
                                })
                                .unwrap_or(ValueShape::Unknown),
                        )
                    }),
                }),
                _ => None,
            },
            "to_error_option" => match receiver {
                ValueShape::Result { err, .. } => Some(ValueShape::Option(
                    err.unwrap_or(Box::new(ValueShape::Unknown)),
                )),
                _ => None,
            },
            "to_option" => match receiver {
                ValueShape::Result { ok, .. } => Some(ValueShape::Option(
                    ok.unwrap_or(Box::new(ValueShape::Unknown)),
                )),
                _ => None,
            },
            "flatten" => match receiver {
                ValueShape::Option(value) => match *value {
                    ValueShape::Option(inner) => Some(ValueShape::Option(inner)),
                    value => Some(ValueShape::Option(Box::new(value))),
                },
                ValueShape::Result { ok, err } => match ok.map(|ok| *ok) {
                    Some(ValueShape::Result { ok, err: inner_err }) => Some(ValueShape::Result {
                        ok,
                        err: inner_err.or(err),
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

    pub(in crate::compiler) fn syntax_callback_return_shape(
        &self,
        receiver: &ValueShape,
        method: &str,
        args: &[SyntaxArgument],
        source: Option<SourceId>,
    ) -> Option<ValueShape> {
        let expression = args.first()?.expression()?;
        let lambda = expression.as_lambda()?;
        let source = source?;
        let lambda_span = syntax_expression_span(source, &expression);
        let params = self
            .lambda_params_from_hir(self.hir_lambda_body(lambda_span).ok()?)
            .ok()?
            .into_iter()
            .map(|param| param.name)
            .collect::<Vec<_>>();
        let hints = super::callback_param_shapes(receiver, method, params.len())?;
        let local_shapes = params
            .into_iter()
            .zip(hints)
            .filter_map(|(name, shape)| shape.map(|shape| (name, shape)))
            .collect::<BTreeMap<_, _>>();
        let body = lambda.body_expression()?;
        self.value_shape_for_syntax_expression_with_locals(Some(source), &body, &local_shapes)
    }

    fn value_shape_for_syntax_expression_with_locals(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
        local_shapes: &BTreeMap<String, ValueShape>,
    ) -> Option<ValueShape> {
        match expression.expression_kind() {
            SyntaxExpressionKind::Literal => self.literal_shape(expression),
            SyntaxExpressionKind::Array => {
                self.array_shape_with_locals(source, expression, local_shapes)
            }
            SyntaxExpressionKind::Tuple => {
                self.tuple_shape_with_locals(source, expression, local_shapes)
            }
            SyntaxExpressionKind::Map => {
                self.map_shape_with_locals(source, expression, local_shapes)
            }
            SyntaxExpressionKind::Record => {
                self.record_shape_with_locals(source, expression, local_shapes)
            }
            SyntaxExpressionKind::Path => self
                .local_path_shape(source, expression, local_shapes)
                .or_else(|| self.path_shape(source, expression)),
            SyntaxExpressionKind::Field => {
                self.field_shape_with_locals(source, expression, local_shapes)
            }
            SyntaxExpressionKind::Index => {
                self.index_shape_with_locals(source, expression, local_shapes)
            }
            SyntaxExpressionKind::Paren => {
                let inner = expression.as_paren()?.expression()?;
                self.value_shape_for_syntax_expression_with_locals(source, &inner, local_shapes)
            }
            SyntaxExpressionKind::Binary => self.binary_shape(expression),
            SyntaxExpressionKind::Try => {
                let operand = expression.as_try()?.expression()?;
                unwrap_try_shape(self.value_shape_for_syntax_expression_with_locals(
                    source,
                    &operand,
                    local_shapes,
                )?)
            }
            SyntaxExpressionKind::Call => {
                self.call_shape_with_locals(source, expression, local_shapes)
            }
            SyntaxExpressionKind::Unit
            | SyntaxExpressionKind::Unary
            | SyntaxExpressionKind::Assign
            | SyntaxExpressionKind::Lambda
            | SyntaxExpressionKind::Block
            | SyntaxExpressionKind::If
            | SyntaxExpressionKind::Match => None,
        }
    }

    fn local_path_shape(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
        local_shapes: &BTreeMap<String, ValueShape>,
    ) -> Option<ValueShape> {
        let source = source?;
        let span = syntax_expression_span(source, expression);
        let path = self.hir_value_path_for_span(span)?;
        let [name] = path.as_slice() else {
            return None;
        };
        local_shapes.get(name).cloned()
    }

    fn array_shape_with_locals(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
        local_shapes: &BTreeMap<String, ValueShape>,
    ) -> Option<ValueShape> {
        let values = expression.as_array()?.expressions().collect::<Vec<_>>();
        if values.is_empty() {
            return Some(ValueShape::Array(Box::new(ValueShape::Unknown)));
        }
        let mut shapes = values
            .iter()
            .map(|value| {
                self.value_shape_for_syntax_expression_with_locals(source, value, local_shapes)
                    .unwrap_or(ValueShape::Unknown)
            })
            .collect::<Vec<_>>();
        let first = shapes.pop()?;
        let element = if shapes.iter().all(|shape| shape == &first) {
            first
        } else {
            ValueShape::Unknown
        };
        Some(ValueShape::Array(Box::new(element)))
    }

    fn tuple_shape_with_locals(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
        local_shapes: &BTreeMap<String, ValueShape>,
    ) -> Option<ValueShape> {
        let elements = expression
            .as_tuple()?
            .expressions()
            .map(|value| {
                self.value_shape_for_syntax_expression_with_locals(source, &value, local_shapes)
                    .unwrap_or(ValueShape::Unknown)
            })
            .collect::<Vec<_>>();
        Some(ValueShape::Tuple(elements))
    }

    fn map_shape_with_locals(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
        local_shapes: &BTreeMap<String, ValueShape>,
    ) -> Option<ValueShape> {
        let entries = expression.as_map()?.entries().collect::<Vec<_>>();
        if entries.is_empty() {
            return Some(ValueShape::Map {
                key: Box::new(ValueShape::Unknown),
                value: Box::new(ValueShape::Unknown),
            });
        }
        let mut keys = entries
            .iter()
            .map(|entry| {
                entry.key().and_then(|key| {
                    self.value_shape_for_syntax_expression_with_locals(source, &key, local_shapes)
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let key = keys.pop()?;
        if !keys.iter().all(|shape| shape == &key) {
            return None;
        }
        let values = entries
            .iter()
            .map(|entry| {
                entry.value().and_then(|value| {
                    self.value_shape_for_syntax_expression_with_locals(source, &value, local_shapes)
                })
            })
            .collect::<Option<Vec<_>>>();
        let value = values.and_then(common_shape).unwrap_or(ValueShape::Unknown);
        Some(ValueShape::Map {
            key: Box::new(key),
            value: Box::new(value),
        })
    }

    fn record_shape_with_locals(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
        local_shapes: &BTreeMap<String, ValueShape>,
    ) -> Option<ValueShape> {
        let record = expression.as_record()?;
        if self.is_enum_variant_record_literal(source, expression) {
            return None;
        }
        let type_name = self.record_literal_type_name(source, expression);
        let fields = record.fields();
        let mut field_names = fields
            .iter()
            .filter_map(|field| field.label_text())
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
            .into_iter()
            .filter_map(|field| {
                let name = field.label_text()?;
                let slot = slots.get(&name).copied()?;
                let value = field.expression().and_then(|value| {
                    self.value_shape_for_syntax_expression_with_locals(source, &value, local_shapes)
                });
                let value_type = value.as_ref().and_then(ValueShape::value_type);
                Some((
                    name,
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

    fn field_shape_with_locals(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
        local_shapes: &BTreeMap<String, ValueShape>,
    ) -> Option<ValueShape> {
        let source = source?;
        let span = syntax_expression_span(source, expression);
        let field = self.hir_field_for_span(span)?;
        let receiver_span = self.expression_span(field.receiver)?;
        let receiver = syntax_shape_expression_at_span(source, expression, receiver_span)?;
        self.value_shape_for_syntax_expression_with_locals(Some(source), &receiver, local_shapes)?
            .as_record()?
            .field_value_shape(&field.name)
            .cloned()
    }

    fn index_shape_with_locals(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
        local_shapes: &BTreeMap<String, ValueShape>,
    ) -> Option<ValueShape> {
        let source = source?;
        let span = syntax_expression_span(source, expression);
        let index = self.hir_index_for_span(span)?;
        let receiver_span = self.expression_span(index.receiver)?;
        let receiver = syntax_shape_expression_at_span(source, expression, receiver_span)?;
        match self.value_shape_for_syntax_expression_with_locals(
            Some(source),
            &receiver,
            local_shapes,
        )? {
            ValueShape::Array(element) => Some(*element),
            ValueShape::Map { value, .. } => Some(*value),
            _ => None,
        }
    }

    fn call_shape_with_locals(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
        local_shapes: &BTreeMap<String, ValueShape>,
    ) -> Option<ValueShape> {
        let source_id = source?;
        let call_expression =
            self.expression_at_span(syntax_expression_span(source_id, expression))?;
        let callee = self.call_callee_expression(call_expression)?;
        let field = self.hir_field_for_expression(callee)?;
        let receiver_span = self.expression_span(field.receiver)?;
        let receiver = syntax_shape_expression_at_span(source_id, expression, receiver_span)?;
        let receiver = self.value_shape_for_syntax_expression_with_locals(
            Some(source_id),
            &receiver,
            local_shapes,
        )?;
        let method = field.name.as_str();
        match method {
            "to_upper" | "to_lower" | "trim" | "trim_start" | "trim_end" | "replace" | "repeat"
            | "join" => Some(string_shape()),
            "len" | "count" | "sum" => Some(ValueShape::Scalar("i64".to_owned())),
            "has" | "contains" | "starts_with" | "ends_with" | "is_empty" | "is_none"
            | "is_some" | "is_ok" | "is_err" | "any" | "all" | "is_subset" | "is_superset"
            | "is_disjoint" => Some(ValueShape::Scalar("bool".to_owned())),
            "first" | "last" | "pop" | "remove_at" | "min" | "max" => receiver
                .array_element()
                .cloned()
                .map(|element| ValueShape::Option(Box::new(element))),
            "values" => match &receiver {
                ValueShape::Array(value)
                | ValueShape::Set(value)
                | ValueShape::Map { value, .. } => Some(ValueShape::Iterator(value.clone())),
                _ => None,
            },
            "collect_array" => receiver
                .iterator_item()
                .cloned()
                .map(|item| ValueShape::Array(Box::new(item))),
            "sort"
            | "sort_by"
            | "reverse"
            | "distinct"
            | "merge"
            | "union"
            | "intersection"
            | "difference"
            | "symmetric_difference" => Some(receiver),
            _ => None,
        }
    }

    fn value_type_for_syntax_expression(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<RuntimeTypeFact> {
        self.value_shape_for_syntax_expression(source, expression)?
            .value_type()
    }

    fn record_literal_type_name(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<String> {
        let source = source?;
        let expression = self.expression_at_span(syntax_expression_span(source, expression))?;
        self.type_symbol_for_expression(expression).or_else(|| {
            self.hir_constructor_path(expression)
                .filter(|path| !path.is_empty())
                .map(|path| path.join("::"))
        })
    }

    fn is_enum_variant_record_literal(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> bool {
        let Some(source) = source else {
            return false;
        };
        let Some(expression) = self.expression_at_span(syntax_expression_span(source, expression))
        else {
            return false;
        };
        self.hir_constructor_path(expression)
            .is_some_and(|path| crate::compiler::patterns::enum_variant_path(path).is_some())
    }
}

fn io_error_shape() -> ValueShape {
    ValueShape::Record(RecordShape::from_field_shapes([
        ("kind".to_owned(), string_shape()),
        ("message".to_owned(), string_shape()),
        ("path".to_owned(), string_shape()),
    ]))
}

fn string_shape() -> ValueShape {
    ValueShape::Scalar("String".to_owned())
}

fn arithmetic_shape(binary: &vela_syntax::ast::SyntaxBinaryExpr) -> Option<String> {
    let left = syntax_numeric_literal_kind(&binary.lhs()?)?;
    let right = syntax_numeric_literal_kind(&binary.rhs()?)?;
    Some(if left_float_or_right_float(left, right) {
        "f64".to_owned()
    } else {
        "i64".to_owned()
    })
}

fn syntax_numeric_literal_kind(expression: &SyntaxExpression) -> Option<NumericLiteralKind> {
    expression
        .as_literal()
        .and_then(|literal| literal.literal())
        .and_then(|literal| match literal {
            Literal::Integer(_) => Some(NumericLiteralKind::Integer),
            Literal::Float(_) => Some(NumericLiteralKind::Float),
            _ => None,
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NumericLiteralKind {
    Integer,
    Float,
}

fn left_float_or_right_float(left: NumericLiteralKind, right: NumericLiteralKind) -> bool {
    matches!(left, NumericLiteralKind::Float) || matches!(right, NumericLiteralKind::Float)
}

fn literal_type(literal: Literal) -> RuntimeTypeFact {
    match literal {
        Literal::Bool(_) => RuntimeTypeFact::primitive(PrimitiveTag::Bool),
        Literal::Char(_) => RuntimeTypeFact::primitive(PrimitiveTag::Char),
        Literal::Integer(value) => RuntimeTypeFact::primitive(match value.suffix {
            Some(vela_syntax::ast::IntegerSuffix::I8) => PrimitiveTag::I8,
            Some(vela_syntax::ast::IntegerSuffix::I16) => PrimitiveTag::I16,
            Some(vela_syntax::ast::IntegerSuffix::I32) => PrimitiveTag::I32,
            None | Some(vela_syntax::ast::IntegerSuffix::I64) => PrimitiveTag::I64,
            Some(vela_syntax::ast::IntegerSuffix::U8) => PrimitiveTag::U8,
            Some(vela_syntax::ast::IntegerSuffix::U16) => PrimitiveTag::U16,
            Some(vela_syntax::ast::IntegerSuffix::U32) => PrimitiveTag::U32,
            Some(vela_syntax::ast::IntegerSuffix::U64) => PrimitiveTag::U64,
        }),
        Literal::Float(value) => RuntimeTypeFact::primitive(match value.suffix {
            Some(vela_syntax::ast::FloatSuffix::F32) => PrimitiveTag::F32,
            None | Some(vela_syntax::ast::FloatSuffix::F64) => PrimitiveTag::F64,
        }),
        Literal::String(_) => RuntimeTypeFact::primitive(PrimitiveTag::String),
        Literal::Bytes(_) => RuntimeTypeFact::primitive(PrimitiveTag::Bytes),
    }
}

fn unwrap_try_shape(shape: ValueShape) -> Option<ValueShape> {
    match shape {
        ValueShape::Option(value) => Some(*value),
        ValueShape::Result { ok: Some(ok), .. } => Some(*ok),
        ValueShape::Result { .. } => Some(ValueShape::Unknown),
        _ => None,
    }
}

fn syntax_expression_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}

fn syntax_shape_expression_at_span(
    source: SourceId,
    expression: &SyntaxExpression,
    span: Span,
) -> Option<SyntaxExpression> {
    if span.source != source {
        return None;
    }
    expression
        .syntax()
        .descendants()
        .filter_map(SyntaxExpression::cast)
        .find(|child| syntax_expression_span(source, child) == span)
}
