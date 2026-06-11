use std::collections::HashMap;

use vela_common::{PrimitiveTag, Span};
use vela_hir::binding::{BindingMap, BindingResolution};
use vela_hir::ids::HirLocalId;
use vela_hir::type_hint::HirTypeHint;
use vela_syntax::ast::{BinaryOp, Expr, ExprKind, Literal};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RuntimeTypeFact {
    Primitive(PrimitiveTag),
    Standard(StandardRuntimeType),
}

pub(super) type TypeRef = RuntimeTypeFact;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum StaticExprType {
    Exact(TypeRef),
    UnsuffixedIntegerLiteral,
    UnsuffixedFloatLiteral,
    Dynamic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StandardRuntimeType {
    Array,
    Map,
    Set,
    Range,
    Function,
    Closure,
    Option,
    Result,
}

impl RuntimeTypeFact {
    pub(super) const fn primitive(tag: PrimitiveTag) -> Self {
        Self::Primitive(tag)
    }

    pub(super) const fn standard(ty: StandardRuntimeType) -> Self {
        Self::Standard(ty)
    }

    pub(super) const fn std_type_name(&self) -> &'static str {
        match self {
            Self::Primitive(PrimitiveTag::Null) => "Null",
            Self::Primitive(PrimitiveTag::Bool) => "Bool",
            Self::Primitive(PrimitiveTag::I8) => "I8",
            Self::Primitive(PrimitiveTag::I16) => "I16",
            Self::Primitive(PrimitiveTag::I32) => "I32",
            Self::Primitive(PrimitiveTag::I64) => "I64",
            Self::Primitive(PrimitiveTag::U8) => "U8",
            Self::Primitive(PrimitiveTag::U16) => "U16",
            Self::Primitive(PrimitiveTag::U32) => "U32",
            Self::Primitive(PrimitiveTag::U64) => "U64",
            Self::Primitive(PrimitiveTag::F32) => "F32",
            Self::Primitive(PrimitiveTag::F64) => "F64",
            Self::Primitive(PrimitiveTag::String) => "String",
            Self::Primitive(PrimitiveTag::Bytes) => "Bytes",
            Self::Standard(StandardRuntimeType::Array) => "Array",
            Self::Standard(StandardRuntimeType::Map) => "Map",
            Self::Standard(StandardRuntimeType::Set) => "Set",
            Self::Standard(StandardRuntimeType::Range) => "Range",
            Self::Standard(StandardRuntimeType::Function) => "Function",
            Self::Standard(StandardRuntimeType::Closure) => "Closure",
            Self::Standard(StandardRuntimeType::Option) => "Option",
            Self::Standard(StandardRuntimeType::Result) => "Result",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ValueTypeFlow {
    locals: HashMap<HirLocalId, RuntimeTypeFact>,
    names: HashMap<String, RuntimeTypeFact>,
}

impl ValueTypeFlow {
    pub(super) fn local_at_span(
        &self,
        bindings: &BindingMap,
        span: Span,
    ) -> Option<RuntimeTypeFact> {
        let BindingResolution::Local(local) = bindings.resolution_at_span(span)? else {
            return None;
        };
        self.local(*local)
    }

    pub(super) fn local(&self, local: HirLocalId) -> Option<RuntimeTypeFact> {
        self.locals.get(&local).cloned()
    }

    pub(super) fn name(&self, name: &str) -> Option<RuntimeTypeFact> {
        self.names.get(name).cloned()
    }

    pub(super) fn set_name(&mut self, name: impl Into<String>, fact: Option<RuntimeTypeFact>) {
        let name = name.into();
        match fact {
            Some(fact) => {
                self.names.insert(name, fact);
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
        fact: Option<RuntimeTypeFact>,
    ) {
        let name = name.into();
        match fact {
            Some(fact) => {
                self.locals.insert(local, fact.clone());
                self.names.insert(name, fact);
            }
            None => {
                self.locals.remove(&local);
                self.names.remove(&name);
            }
        }
    }
}

pub(super) fn expression_value_type(
    expr: &Expr,
    local_type_at_span: impl Fn(Span) -> Option<RuntimeTypeFact>,
    local_type_named: impl Fn(&str) -> Option<RuntimeTypeFact>,
) -> Option<RuntimeTypeFact> {
    match static_expr_type(expr, local_type_at_span, local_type_named) {
        StaticExprType::Exact(fact) => Some(fact),
        StaticExprType::UnsuffixedIntegerLiteral => {
            Some(RuntimeTypeFact::primitive(PrimitiveTag::I64))
        }
        StaticExprType::UnsuffixedFloatLiteral => {
            Some(RuntimeTypeFact::primitive(PrimitiveTag::F64))
        }
        StaticExprType::Dynamic => None,
    }
}

pub(super) fn static_expr_type(
    expr: &Expr,
    local_type_at_span: impl Fn(Span) -> Option<RuntimeTypeFact>,
    local_type_named: impl Fn(&str) -> Option<RuntimeTypeFact>,
) -> StaticExprType {
    match &expr.kind {
        ExprKind::Literal(Literal::Integer(value)) if value.suffix.is_none() => {
            StaticExprType::UnsuffixedIntegerLiteral
        }
        ExprKind::Literal(Literal::Float(value)) if value.suffix.is_none() => {
            StaticExprType::UnsuffixedFloatLiteral
        }
        ExprKind::Literal(Literal::Null) => {
            StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::Null))
        }
        ExprKind::Literal(Literal::Bool(_)) => {
            StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::Bool))
        }
        ExprKind::Literal(Literal::Integer(value)) => {
            StaticExprType::Exact(RuntimeTypeFact::primitive(integer_literal_tag(value)))
        }
        ExprKind::Literal(Literal::Float(value)) => {
            StaticExprType::Exact(RuntimeTypeFact::primitive(float_literal_tag(value)))
        }
        ExprKind::Literal(Literal::String(_)) => {
            StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::String))
        }
        ExprKind::Literal(Literal::Bytes(_)) => {
            StaticExprType::Exact(RuntimeTypeFact::primitive(PrimitiveTag::Bytes))
        }
        ExprKind::Array(_) => {
            StaticExprType::Exact(RuntimeTypeFact::standard(StandardRuntimeType::Array))
        }
        ExprKind::Map(_) => {
            StaticExprType::Exact(RuntimeTypeFact::standard(StandardRuntimeType::Map))
        }
        ExprKind::Lambda { .. } => {
            StaticExprType::Exact(RuntimeTypeFact::standard(StandardRuntimeType::Closure))
        }
        ExprKind::Binary {
            op: BinaryOp::Range,
            ..
        } => StaticExprType::Exact(RuntimeTypeFact::standard(StandardRuntimeType::Range)),
        ExprKind::Path(path) => local_type_at_span(expr.span)
            .or_else(|| {
                path.as_slice()
                    .first()
                    .and_then(|name| (path.len() == 1).then(|| local_type_named(name)).flatten())
            })
            .map(StaticExprType::Exact)
            .unwrap_or(StaticExprType::Dynamic),
        ExprKind::SelfValue => local_type_at_span(expr.span)
            .or_else(|| local_type_named("self"))
            .map(StaticExprType::Exact)
            .unwrap_or(StaticExprType::Dynamic),
        _ => StaticExprType::Dynamic,
    }
}

pub(super) fn type_hint_value_type(hint: &HirTypeHint) -> Option<RuntimeTypeFact> {
    match hint.display().as_str() {
        "null" => Some(RuntimeTypeFact::primitive(PrimitiveTag::Null)),
        "bool" => Some(RuntimeTypeFact::primitive(PrimitiveTag::Bool)),
        "i8" => Some(RuntimeTypeFact::primitive(PrimitiveTag::I8)),
        "i16" => Some(RuntimeTypeFact::primitive(PrimitiveTag::I16)),
        "i32" => Some(RuntimeTypeFact::primitive(PrimitiveTag::I32)),
        "i64" => Some(RuntimeTypeFact::primitive(PrimitiveTag::I64)),
        "u8" => Some(RuntimeTypeFact::primitive(PrimitiveTag::U8)),
        "u16" => Some(RuntimeTypeFact::primitive(PrimitiveTag::U16)),
        "u32" => Some(RuntimeTypeFact::primitive(PrimitiveTag::U32)),
        "u64" => Some(RuntimeTypeFact::primitive(PrimitiveTag::U64)),
        "f32" => Some(RuntimeTypeFact::primitive(PrimitiveTag::F32)),
        "f64" => Some(RuntimeTypeFact::primitive(PrimitiveTag::F64)),
        "string" => Some(RuntimeTypeFact::primitive(PrimitiveTag::String)),
        "bytes" => Some(RuntimeTypeFact::primitive(PrimitiveTag::Bytes)),
        "array" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Array)),
        "map" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Map)),
        "set" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Set)),
        "range" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Range)),
        "function" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Function)),
        "closure" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Closure)),
        "Option" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Option)),
        "Result" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Result)),
        _ => None,
    }
}

fn integer_literal_tag(value: &vela_syntax::ast::IntegerLiteral) -> PrimitiveTag {
    match value.suffix {
        Some(vela_syntax::ast::IntegerSuffix::I8) => PrimitiveTag::I8,
        Some(vela_syntax::ast::IntegerSuffix::I16) => PrimitiveTag::I16,
        Some(vela_syntax::ast::IntegerSuffix::I32) => PrimitiveTag::I32,
        None | Some(vela_syntax::ast::IntegerSuffix::I64) => PrimitiveTag::I64,
        Some(vela_syntax::ast::IntegerSuffix::U8) => PrimitiveTag::U8,
        Some(vela_syntax::ast::IntegerSuffix::U16) => PrimitiveTag::U16,
        Some(vela_syntax::ast::IntegerSuffix::U32) => PrimitiveTag::U32,
        Some(vela_syntax::ast::IntegerSuffix::U64) => PrimitiveTag::U64,
    }
}

fn float_literal_tag(value: &vela_syntax::ast::FloatLiteral) -> PrimitiveTag {
    match value.suffix {
        Some(vela_syntax::ast::FloatSuffix::F32) => PrimitiveTag::F32,
        None | Some(vela_syntax::ast::FloatSuffix::F64) => PrimitiveTag::F64,
    }
}

impl super::Compiler<'_, '_> {
    pub(super) fn value_type_for_expr(&self, expr: &Expr) -> Option<RuntimeTypeFact> {
        match self.static_type_for_expr(expr) {
            StaticExprType::Exact(fact) => Some(fact),
            StaticExprType::UnsuffixedIntegerLiteral => {
                Some(RuntimeTypeFact::primitive(PrimitiveTag::I64))
            }
            StaticExprType::UnsuffixedFloatLiteral => {
                Some(RuntimeTypeFact::primitive(PrimitiveTag::F64))
            }
            StaticExprType::Dynamic => None,
        }
    }

    pub(super) fn static_type_for_expr(&self, expr: &Expr) -> StaticExprType {
        match static_expr_type(
            expr,
            |span| self.value_types.local_at_span(self.bindings, span),
            |name| self.value_types.name(name),
        ) {
            StaticExprType::Dynamic => self
                .record_field_value_type_for_expr(expr)
                .map(StaticExprType::Exact)
                .unwrap_or(StaticExprType::Dynamic),
            known => known,
        }
    }
}
