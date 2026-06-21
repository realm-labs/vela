use std::collections::BTreeMap;

use vela_common::{PrimitiveTag, SourceId, Span};
use vela_syntax::SyntaxKind;
use vela_syntax::ast::{AstNode, Literal, SyntaxExpression, SyntaxExpressionKind};

use crate::compiler::Compiler;
use crate::compiler::body_payloads::CompilerExpressionPayload;
use crate::compiler::value_types::RuntimeTypeFact;

use super::{RecordFieldShape, RecordShape, ValueShape, common_shape};

impl Compiler<'_, '_> {
    pub(super) fn value_shape_for_syntax_payload(
        &self,
        payload: &CompilerExpressionPayload<'_>,
    ) -> Option<ValueShape> {
        self.value_shape_for_syntax_expression(payload.source(), payload.syntax_expression()?)
    }

    fn value_shape_for_syntax_expression(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<ValueShape> {
        match expression.expression_kind() {
            SyntaxExpressionKind::Literal => self.literal_shape(expression),
            SyntaxExpressionKind::Array => self.array_shape(source, expression),
            SyntaxExpressionKind::Map => self.map_shape(source, expression),
            SyntaxExpressionKind::Record => self.record_shape(source, expression),
            SyntaxExpressionKind::Path => self.path_shape(source, expression),
            SyntaxExpressionKind::Field => self.field_shape(source, expression),
            SyntaxExpressionKind::Paren
            | SyntaxExpressionKind::Unary
            | SyntaxExpressionKind::Binary
            | SyntaxExpressionKind::Assign
            | SyntaxExpressionKind::Call
            | SyntaxExpressionKind::Index
            | SyntaxExpressionKind::Try
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
        let path = record.path_segments();
        if path.len() > 1 {
            return None;
        }
        let type_name = path.first().cloned();
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
        let path = expression.as_path()?.path_segments();
        let local_shape = source
            .map(|source| syntax_expression_span(source, expression))
            .and_then(|span| self.value_shapes.local_at_span(self.bindings, span))
            .or_else(|| {
                source
                    .map(|source| syntax_expression_span(source, expression))
                    .and_then(|span| self.script_types.local_at_span(self.bindings, span))
                    .and_then(|type_name| self.record_shape_for_type(&type_name))
                    .map(ValueShape::Record)
            });
        let [root] = path.as_slice() else {
            return local_shape;
        };
        local_shape.or_else(|| self.shape_named(root))
    }

    fn field_shape(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<ValueShape> {
        let field = expression.as_field()?;
        let receiver = field.receiver()?;
        let name = field.name_text()?;
        self.value_shape_for_syntax_expression(source, &receiver)?
            .as_record()?
            .field_value_shape(&name)
            .cloned()
    }

    fn value_type_for_syntax_expression(
        &self,
        source: Option<SourceId>,
        expression: &SyntaxExpression,
    ) -> Option<RuntimeTypeFact> {
        self.value_shape_for_syntax_expression(source, expression)?
            .value_type()
    }

    fn shape_named(&self, name: &str) -> Option<ValueShape> {
        self.value_shapes.name(name).or_else(|| {
            self.script_types
                .name(name)
                .or_else(|| self.global_type_named(name))
                .and_then(|type_name| self.record_shape_for_type(&type_name))
                .map(ValueShape::Record)
        })
    }
}

fn literal_type(literal: Literal) -> RuntimeTypeFact {
    match literal {
        Literal::Null => RuntimeTypeFact::primitive(PrimitiveTag::Null),
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

fn syntax_expression_span(source: SourceId, expression: &SyntaxExpression) -> Span {
    let range = expression.syntax().text_range();
    Span::new(source, range.start().into(), range.end().into())
}
