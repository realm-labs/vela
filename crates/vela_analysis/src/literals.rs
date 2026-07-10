//! Numeric-literal validation shared by semantic analysis and compiler backends.
//!
//! Literal spelling and range checks belong here so executable lowering and
//! compile-time evaluation cannot disagree about suffixes, defaults, or signed
//! minimum values. The result contains only language-level primitive values;
//! it has no bytecode or MIR representation knowledge.

use std::collections::{BTreeMap, BTreeSet};
use std::num::{ParseFloatError, ParseIntError};

use vela_common::{Diagnostic, PrimitiveTag, ScalarValue, Span};
use vela_hir::body::{
    HirBinaryOp, HirBody, HirExprKind, HirFloatLiteral, HirFloatSuffix, HirIntRadix,
    HirIntegerLiteral, HirIntegerSuffix, HirLiteral, HirPatternKind, HirUnaryOp,
};
use vela_hir::ids::{HirBodyId, HirExprId, HirPatternId};
use vela_hir::module_graph::ModuleGraph;

/// The primitive-selection policy for an unsuffixed numeric literal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiteralPrimitiveContext {
    /// Use the language defaults (`i64` for integers and `f64` for floats).
    Default,
    /// Convert an unsuffixed literal to this statically required primitive.
    Expected(PrimitiveTag),
    /// Retain a validated, unsuffixed literal for dynamic numeric dispatch.
    DeferredDynamic,
}

/// Whether a directly enclosing unary minus is part of literal validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiteralSign {
    Positive,
    Negated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericLiteralKind {
    Integer,
    Float,
}

/// A source-independent unsuffixed numeric literal used as one expression.
///
/// Outer parentheses remain explicit so compile validation can preserve the
/// established distinction between direct contract literals and inline
/// dynamic-operator literals. Negation is recognized only when its operand is
/// the literal itself, matching the executable surface accepted before MIR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NumericLiteralUse {
    expression: HirExprId,
    resolution_expression: HirExprId,
    literal_expression: HirExprId,
    kind: NumericLiteralKind,
    sign: LiteralSign,
    parenthesized: bool,
}

impl NumericLiteralUse {
    #[must_use]
    pub fn classify(body: &HirBody, expression: HirExprId) -> Option<Self> {
        let mut candidate = expression;
        let mut parenthesized = false;
        loop {
            let record = body.expression(candidate)?;
            match record.kind {
                HirExprKind::Paren {
                    expression: Some(inner),
                } => {
                    candidate = inner;
                    parenthesized = true;
                }
                _ => break,
            }
        }

        let record = body.expression(candidate)?;
        let (literal_expression, kind, sign) = match &record.kind {
            HirExprKind::Literal(literal) => (
                candidate,
                unsuffixed_numeric_literal_kind(literal)?,
                LiteralSign::Positive,
            ),
            HirExprKind::Unary {
                op: Some(HirUnaryOp::Negate),
                operand: Some(operand),
            } => {
                let literal = match &body.expression(*operand)?.kind {
                    HirExprKind::Literal(literal) => literal,
                    _ => return None,
                };
                (
                    *operand,
                    unsuffixed_numeric_literal_kind(literal)?,
                    LiteralSign::Negated,
                )
            }
            _ => return None,
        };
        Some(Self {
            expression,
            resolution_expression: candidate,
            literal_expression,
            kind,
            sign,
            parenthesized,
        })
    }

    #[must_use]
    pub const fn expression(self) -> HirExprId {
        self.expression
    }

    #[must_use]
    pub const fn resolution_expression(self) -> HirExprId {
        self.resolution_expression
    }

    #[must_use]
    pub const fn literal_expression(self) -> HirExprId {
        self.literal_expression
    }

    #[must_use]
    pub const fn kind(self) -> NumericLiteralKind {
        self.kind
    }

    #[must_use]
    pub const fn sign(self) -> LiteralSign {
        self.sign
    }

    #[must_use]
    pub const fn is_parenthesized(self) -> bool {
        self.parenthesized
    }

    /// Direct expected-contract contextualization historically did not look
    /// through parentheses. Keep that migration baseline explicit.
    #[must_use]
    pub const fn supports_direct_contract_context(self) -> bool {
        !self.parenthesized
    }

    /// Dynamic contextual numeric operations accept only positive literals
    /// and the arithmetic/ordering family supported by the runtime.
    #[must_use]
    pub const fn supports_deferred_operation(self, operation: HirBinaryOp) -> bool {
        matches!(self.sign, LiteralSign::Positive) && supports_deferred_numeric_literal(operation)
    }
}

#[must_use]
pub const fn supports_deferred_numeric_literal(operation: HirBinaryOp) -> bool {
    matches!(
        operation,
        HirBinaryOp::Add
            | HirBinaryOp::Sub
            | HirBinaryOp::Mul
            | HirBinaryOp::Div
            | HirBinaryOp::Rem
            | HirBinaryOp::Less
            | HirBinaryOp::LessEqual
            | HirBinaryOp::Greater
            | HirBinaryOp::GreaterEqual
    )
}

impl NumericLiteralKind {
    #[must_use]
    pub const fn accepts_primitive(self, primitive: PrimitiveTag) -> bool {
        match (self, primitive.numeric_tag()) {
            (Self::Integer, Some(tag)) => tag.is_integer(),
            (Self::Float, Some(tag)) => tag.is_float(),
            (Self::Integer | Self::Float, None) => false,
        }
    }
}

/// A validated unsuffixed literal whose final primitive follows a dynamic
/// numeric operand at runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeferredNumericLiteral {
    kind: NumericLiteralKind,
    text: String,
}

impl DeferredNumericLiteral {
    #[must_use]
    pub const fn kind(&self) -> NumericLiteralKind {
        self.kind
    }

    /// Returns the suffix-free source spelling retained by the runtime
    /// contextual-numeric operation.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// A scalar value that has passed literal syntax, primitive, and range checks.
#[derive(Clone, Copy, Debug)]
pub struct ValidatedScalar(ScalarValue);

impl ValidatedScalar {
    #[must_use]
    pub const fn value(self) -> ScalarValue {
        self.0
    }

    #[must_use]
    pub const fn primitive(self) -> PrimitiveTag {
        self.0.primitive_tag()
    }
}

impl PartialEq for ValidatedScalar {
    fn eq(&self, other: &Self) -> bool {
        match (self.0, other.0) {
            (ScalarValue::I8(left), ScalarValue::I8(right)) => left == right,
            (ScalarValue::I16(left), ScalarValue::I16(right)) => left == right,
            (ScalarValue::I32(left), ScalarValue::I32(right)) => left == right,
            (ScalarValue::I64(left), ScalarValue::I64(right)) => left == right,
            (ScalarValue::U8(left), ScalarValue::U8(right)) => left == right,
            (ScalarValue::U16(left), ScalarValue::U16(right)) => left == right,
            (ScalarValue::U32(left), ScalarValue::U32(right)) => left == right,
            (ScalarValue::U64(left), ScalarValue::U64(right)) => left == right,
            (ScalarValue::F32(left), ScalarValue::F32(right)) => left.to_bits() == right.to_bits(),
            (ScalarValue::F64(left), ScalarValue::F64(right)) => left.to_bits() == right.to_bits(),
            _ => false,
        }
    }
}

impl Eq for ValidatedScalar {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedLiteralFact {
    Scalar(ValidatedScalar),
    Deferred(DeferredNumericLiteral),
}

impl ResolvedLiteralFact {
    #[must_use]
    pub const fn scalar(&self) -> Option<ScalarValue> {
        match self {
            Self::Scalar(value) => Some(value.value()),
            Self::Deferred(_) => None,
        }
    }

    #[must_use]
    pub const fn deferred(&self) -> Option<&DeferredNumericLiteral> {
        match self {
            Self::Scalar(_) => None,
            Self::Deferred(value) => Some(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiteralErrorClass {
    InvalidDigits,
    OutOfRange,
    IncompatiblePrimitive,
}

/// A backend-neutral literal error. Its compiler diagnostic projection keeps
/// the established public code and message contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiteralError {
    kind: NumericLiteralKind,
    class: LiteralErrorClass,
    spelling: String,
    detail: String,
}

impl LiteralError {
    #[must_use]
    pub const fn kind(&self) -> NumericLiteralKind {
        self.kind
    }

    #[must_use]
    pub const fn class(&self) -> LiteralErrorClass {
        self.class
    }

    #[must_use]
    pub fn spelling(&self) -> &str {
        &self.spelling
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    #[must_use]
    pub fn to_compiler_diagnostic(&self, span: Span) -> Option<Diagnostic> {
        if self.class == LiteralErrorClass::IncompatiblePrimitive {
            return None;
        }
        let (noun, code) = match self.kind {
            NumericLiteralKind::Integer => ("integer", "compiler::invalid_int_literal"),
            NumericLiteralKind::Float => ("float", "compiler::invalid_float_literal"),
        };
        Some(
            Diagnostic::error(format!(
                "invalid {noun} literal `{}`: {}",
                self.spelling, self.detail
            ))
            .with_code(code)
            .with_span(span),
        )
    }
}

pub type LiteralResult = Result<ResolvedLiteralFact, LiteralError>;

/// Numeric literal facts indexed by both a literal expression and its direct
/// unary-negation expression when the negation is folded into the value.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiteralFacts {
    facts: BTreeMap<HirExprId, LiteralResult>,
    patterns: BTreeMap<HirPatternId, LiteralResult>,
    diagnostic_origins: BTreeMap<HirExprId, HirExprId>,
}

impl LiteralFacts {
    #[must_use]
    pub fn from_module_graph(graph: &ModuleGraph) -> Self {
        Self::from_module_graph_with_contexts(graph, &BTreeMap::new())
    }

    /// Re-resolves literals using exact primitive/dynamic contexts keyed by
    /// HIR expression. A context may name either the literal itself or its
    /// directly enclosing unary-negation expression.
    #[must_use]
    pub fn from_module_graph_with_contexts(
        graph: &ModuleGraph,
        contexts: &BTreeMap<HirExprId, LiteralPrimitiveContext>,
    ) -> Self {
        Self::from_module_graph_with_body_filter(graph, None, contexts)
    }

    pub(crate) fn from_module_graph_for_bodies_with_contexts(
        graph: &ModuleGraph,
        bodies: &BTreeSet<HirBodyId>,
        contexts: &BTreeMap<HirExprId, LiteralPrimitiveContext>,
    ) -> Self {
        Self::from_module_graph_with_body_filter(graph, Some(bodies), contexts)
    }

    fn from_module_graph_with_body_filter(
        graph: &ModuleGraph,
        bodies: Option<&BTreeSet<HirBodyId>>,
        contexts: &BTreeMap<HirExprId, LiteralPrimitiveContext>,
    ) -> Self {
        let mut facts = BTreeMap::new();
        let mut patterns = BTreeMap::new();
        let mut diagnostic_origins = BTreeMap::new();
        for body in graph
            .bodies()
            .filter(|body| bodies.is_none_or(|bodies| bodies.contains(&body.id)))
        {
            let negated_operands: BTreeMap<_, _> = body
                .expressions
                .iter()
                .filter_map(|(expression, record)| match record.kind {
                    HirExprKind::Unary {
                        op: Some(HirUnaryOp::Negate),
                        operand: Some(operand),
                    } => Some((operand, *expression)),
                    _ => None,
                })
                .collect();

            for (expression, record) in &body.expressions {
                let HirExprKind::Literal(literal) = &record.kind else {
                    continue;
                };
                if numeric_literal_kind(literal).is_none() {
                    continue;
                }
                let negated_by = negated_operands.get(expression).copied();
                let context = negated_by
                    .and_then(|unary| contexts.get(&unary))
                    .or_else(|| contexts.get(expression))
                    .copied()
                    .unwrap_or(LiteralPrimitiveContext::Default);
                let positive = resolve_numeric_literal(literal, context, LiteralSign::Positive)
                    .expect("numeric literal kind was checked before resolution");
                if let Some(unary) = negated_by
                    && negation_is_literal_semantic(literal, context)
                {
                    if positive.is_ok() {
                        facts.insert(*expression, positive);
                    }
                    let negated = resolve_numeric_literal(literal, context, LiteralSign::Negated)
                        .expect("numeric literal kind was checked before resolution");
                    if negated.is_err() {
                        diagnostic_origins.insert(unary, *expression);
                    }
                    facts.insert(unary, negated);
                } else {
                    facts.insert(*expression, positive);
                }
            }

            for pattern in body.patterns.values() {
                let HirPatternKind::Literal(Some(literal)) = &pattern.kind else {
                    continue;
                };
                if numeric_literal_kind(literal).is_none() {
                    continue;
                }
                let result = resolve_numeric_literal(
                    literal,
                    LiteralPrimitiveContext::Default,
                    LiteralSign::Positive,
                )
                .expect("numeric literal kind was checked before resolution");
                patterns.insert(pattern.id, result);
            }
        }
        Self {
            facts,
            patterns,
            diagnostic_origins,
        }
    }

    #[must_use]
    pub fn get(&self, expression: HirExprId) -> Option<&LiteralResult> {
        self.facts.get(&expression)
    }

    #[must_use]
    pub fn pattern(&self, pattern: HirPatternId) -> Option<&LiteralResult> {
        self.patterns.get(&pattern)
    }

    pub fn errors(&self) -> impl Iterator<Item = (HirExprId, &LiteralError)> {
        self.facts
            .iter()
            .filter_map(|(expression, fact)| fact.as_ref().err().map(|error| (*expression, error)))
    }

    pub fn pattern_errors(&self) -> impl Iterator<Item = (HirPatternId, &LiteralError)> {
        self.patterns
            .iter()
            .filter_map(|(pattern, fact)| fact.as_ref().err().map(|error| (*pattern, error)))
    }

    /// Projects range/parse failures to the frozen compiler diagnostic
    /// contract. Primitive incompatibilities remain owned by expected-contract
    /// analysis and are intentionally excluded here.
    #[must_use]
    pub fn compiler_diagnostics(&self, graph: &ModuleGraph) -> Vec<Diagnostic> {
        let mut diagnostics = self
            .errors()
            .filter_map(|(expression, error)| {
                let span = graph.expression_span(
                    self.diagnostic_origins
                        .get(&expression)
                        .copied()
                        .unwrap_or(expression),
                )?;
                Some((span, error.to_compiler_diagnostic(span)?))
            })
            .chain(self.pattern_errors().filter_map(|(pattern, error)| {
                let span = graph.pattern_span(pattern)?;
                Some((span, error.to_compiler_diagnostic(span)?))
            }))
            .collect::<Vec<_>>();
        diagnostics.sort_by_key(|(span, _)| (span.source, span.start, span.end));
        diagnostics
            .into_iter()
            .map(|(_, diagnostic)| diagnostic)
            .collect()
    }
}

#[must_use]
pub const fn integer_suffix_primitive(suffix: Option<HirIntegerSuffix>) -> PrimitiveTag {
    match suffix {
        None | Some(HirIntegerSuffix::I64) => PrimitiveTag::I64,
        Some(HirIntegerSuffix::I8) => PrimitiveTag::I8,
        Some(HirIntegerSuffix::I16) => PrimitiveTag::I16,
        Some(HirIntegerSuffix::I32) => PrimitiveTag::I32,
        Some(HirIntegerSuffix::U8) => PrimitiveTag::U8,
        Some(HirIntegerSuffix::U16) => PrimitiveTag::U16,
        Some(HirIntegerSuffix::U32) => PrimitiveTag::U32,
        Some(HirIntegerSuffix::U64) => PrimitiveTag::U64,
    }
}

#[must_use]
pub const fn float_suffix_primitive(suffix: Option<HirFloatSuffix>) -> PrimitiveTag {
    match suffix {
        Some(HirFloatSuffix::F32) => PrimitiveTag::F32,
        None | Some(HirFloatSuffix::F64) => PrimitiveTag::F64,
    }
}

#[must_use]
pub fn integer_literal_spelling(value: &HirIntegerLiteral) -> String {
    value.source_spelling()
}

#[must_use]
pub fn float_literal_spelling(value: &HirFloatLiteral) -> String {
    value.source_spelling()
}

/// Resolves a numeric HIR literal using language-level primitive rules.
pub fn resolve_numeric_literal(
    literal: &HirLiteral,
    context: LiteralPrimitiveContext,
    sign: LiteralSign,
) -> Option<LiteralResult> {
    match literal {
        HirLiteral::Integer(value) => Some(resolve_integer_literal(value, context, sign)),
        HirLiteral::Float(value) => Some(resolve_float_literal(value, context, sign)),
        _ => None,
    }
}

pub fn resolve_integer_literal(
    literal: &HirIntegerLiteral,
    context: LiteralPrimitiveContext,
    sign: LiteralSign,
) -> LiteralResult {
    let magnitude = parse_integer_magnitude(literal)?;
    let primitive = select_primitive(
        NumericLiteralKind::Integer,
        integer_literal_spelling(literal),
        literal
            .suffix
            .map(|_| integer_suffix_primitive(literal.suffix)),
        context,
    )?;
    if context == LiteralPrimitiveContext::DeferredDynamic && literal.suffix.is_none() {
        if magnitude > u64::MAX as u128 {
            return Err(out_of_range(
                NumericLiteralKind::Integer,
                integer_literal_spelling(literal),
            ));
        }
        return Ok(ResolvedLiteralFact::Deferred(DeferredNumericLiteral {
            kind: NumericLiteralKind::Integer,
            text: literal.text.clone(),
        }));
    }

    let value = match primitive {
        PrimitiveTag::I8 => {
            ScalarValue::I8(resolve_signed(magnitude, i8::MAX as u128, sign, literal)? as i8)
        }
        PrimitiveTag::I16 => {
            ScalarValue::I16(resolve_signed(magnitude, i16::MAX as u128, sign, literal)? as i16)
        }
        PrimitiveTag::I32 => {
            ScalarValue::I32(resolve_signed(magnitude, i32::MAX as u128, sign, literal)? as i32)
        }
        PrimitiveTag::I64 => {
            ScalarValue::I64(resolve_signed(magnitude, i64::MAX as u128, sign, literal)? as i64)
        }
        PrimitiveTag::U8 => {
            ScalarValue::U8(resolve_unsigned(magnitude, u8::MAX as u128, literal)? as u8)
        }
        PrimitiveTag::U16 => {
            ScalarValue::U16(resolve_unsigned(magnitude, u16::MAX as u128, literal)? as u16)
        }
        PrimitiveTag::U32 => {
            ScalarValue::U32(resolve_unsigned(magnitude, u32::MAX as u128, literal)? as u32)
        }
        PrimitiveTag::U64 => {
            ScalarValue::U64(resolve_unsigned(magnitude, u64::MAX as u128, literal)? as u64)
        }
        _ => {
            return Err(incompatible(
                NumericLiteralKind::Integer,
                integer_literal_spelling(literal),
                primitive,
            ));
        }
    };
    Ok(ResolvedLiteralFact::Scalar(ValidatedScalar(value)))
}

pub fn resolve_float_literal(
    literal: &HirFloatLiteral,
    context: LiteralPrimitiveContext,
    sign: LiteralSign,
) -> LiteralResult {
    let primitive = select_primitive(
        NumericLiteralKind::Float,
        float_literal_spelling(literal),
        literal
            .suffix
            .map(|_| float_suffix_primitive(literal.suffix)),
        context,
    )?;
    if context == LiteralPrimitiveContext::DeferredDynamic && literal.suffix.is_none() {
        let _: f64 = parse_finite_float(literal)?;
        return Ok(ResolvedLiteralFact::Deferred(DeferredNumericLiteral {
            kind: NumericLiteralKind::Float,
            text: literal.text.clone(),
        }));
    }
    let value = match primitive {
        PrimitiveTag::F32 => {
            let value = parse_finite_float::<f32>(literal)?;
            ScalarValue::F32(if sign == LiteralSign::Negated {
                -value
            } else {
                value
            })
        }
        PrimitiveTag::F64 => {
            let value = parse_finite_float::<f64>(literal)?;
            ScalarValue::F64(if sign == LiteralSign::Negated {
                -value
            } else {
                value
            })
        }
        _ => {
            return Err(incompatible(
                NumericLiteralKind::Float,
                float_literal_spelling(literal),
                primitive,
            ));
        }
    };
    Ok(ResolvedLiteralFact::Scalar(ValidatedScalar(value)))
}

fn numeric_literal_kind(literal: &HirLiteral) -> Option<NumericLiteralKind> {
    match literal {
        HirLiteral::Integer(_) => Some(NumericLiteralKind::Integer),
        HirLiteral::Float(_) => Some(NumericLiteralKind::Float),
        _ => None,
    }
}

fn unsuffixed_numeric_literal_kind(literal: &HirLiteral) -> Option<NumericLiteralKind> {
    match literal {
        HirLiteral::Integer(value) if value.suffix.is_none() => Some(NumericLiteralKind::Integer),
        HirLiteral::Float(value) if value.suffix.is_none() => Some(NumericLiteralKind::Float),
        _ => None,
    }
}

fn negation_is_literal_semantic(literal: &HirLiteral, context: LiteralPrimitiveContext) -> bool {
    match literal {
        HirLiteral::Float(_) => context != LiteralPrimitiveContext::DeferredDynamic,
        HirLiteral::Integer(value) => {
            let primitive = value.suffix.map_or_else(
                || match context {
                    LiteralPrimitiveContext::Expected(expected) => expected,
                    LiteralPrimitiveContext::Default | LiteralPrimitiveContext::DeferredDynamic => {
                        PrimitiveTag::I64
                    }
                },
                |suffix| integer_suffix_primitive(Some(suffix)),
            );
            context != LiteralPrimitiveContext::DeferredDynamic
                && primitive
                    .numeric_tag()
                    .is_some_and(|tag| tag.is_signed_integer())
        }
        _ => false,
    }
}

fn select_primitive(
    kind: NumericLiteralKind,
    spelling: String,
    suffix: Option<PrimitiveTag>,
    context: LiteralPrimitiveContext,
) -> Result<PrimitiveTag, LiteralError> {
    let expected = match context {
        LiteralPrimitiveContext::Default | LiteralPrimitiveContext::DeferredDynamic => match kind {
            NumericLiteralKind::Integer => PrimitiveTag::I64,
            NumericLiteralKind::Float => PrimitiveTag::F64,
        },
        LiteralPrimitiveContext::Expected(expected) => expected,
    };
    if let Some(suffix) = suffix
        && context != LiteralPrimitiveContext::Default
        && context != LiteralPrimitiveContext::DeferredDynamic
        && suffix != expected
    {
        return Err(incompatible(kind, spelling, expected));
    }
    let selected = suffix.unwrap_or(expected);
    let compatible = kind.accepts_primitive(selected);
    if compatible {
        Ok(selected)
    } else {
        Err(incompatible(kind, spelling, expected))
    }
}

fn parse_integer_magnitude(literal: &HirIntegerLiteral) -> Result<u128, LiteralError> {
    let text = literal.text.replace('_', "");
    let digits = match literal.radix {
        HirIntRadix::Binary | HirIntRadix::Hex => text.get(2..).unwrap_or_default(),
        HirIntRadix::Decimal => text.as_str(),
    };
    let radix = match literal.radix {
        HirIntRadix::Binary => 2,
        HirIntRadix::Decimal => 10,
        HirIntRadix::Hex => 16,
    };
    u128::from_str_radix(digits, radix).map_err(|error: ParseIntError| LiteralError {
        kind: NumericLiteralKind::Integer,
        class: LiteralErrorClass::InvalidDigits,
        spelling: integer_literal_spelling(literal),
        detail: error.to_string(),
    })
}

fn resolve_signed(
    magnitude: u128,
    positive_max: u128,
    sign: LiteralSign,
    literal: &HirIntegerLiteral,
) -> Result<i128, LiteralError> {
    match sign {
        LiteralSign::Positive if magnitude <= positive_max => Ok(magnitude as i128),
        LiteralSign::Negated if magnitude <= positive_max + 1 => Ok(-(magnitude as i128)),
        LiteralSign::Positive | LiteralSign::Negated => Err(out_of_range(
            NumericLiteralKind::Integer,
            integer_literal_spelling(literal),
        )),
    }
}

fn resolve_unsigned(
    magnitude: u128,
    max: u128,
    literal: &HirIntegerLiteral,
) -> Result<u128, LiteralError> {
    if magnitude <= max {
        Ok(magnitude)
    } else {
        Err(out_of_range(
            NumericLiteralKind::Integer,
            integer_literal_spelling(literal),
        ))
    }
}

fn parse_finite_float<T>(literal: &HirFloatLiteral) -> Result<T, LiteralError>
where
    T: Copy + Into<f64> + std::str::FromStr<Err = ParseFloatError>,
{
    let value: T = literal
        .text
        .replace('_', "")
        .parse()
        .map_err(|error: ParseFloatError| LiteralError {
            kind: NumericLiteralKind::Float,
            class: LiteralErrorClass::InvalidDigits,
            spelling: float_literal_spelling(literal),
            detail: error.to_string(),
        })?;
    if value.into().is_finite() {
        Ok(value)
    } else {
        Err(out_of_range(
            NumericLiteralKind::Float,
            float_literal_spelling(literal),
        ))
    }
}

fn out_of_range(kind: NumericLiteralKind, spelling: String) -> LiteralError {
    LiteralError {
        kind,
        class: LiteralErrorClass::OutOfRange,
        spelling,
        detail: match kind {
            NumericLiteralKind::Integer => "integer literal out of range",
            NumericLiteralKind::Float => "float literal out of range",
        }
        .to_owned(),
    }
}

fn incompatible(
    kind: NumericLiteralKind,
    spelling: String,
    expected: PrimitiveTag,
) -> LiteralError {
    LiteralError {
        kind,
        class: LiteralErrorClass::IncompatiblePrimitive,
        spelling,
        detail: format!("literal is incompatible with `{expected}`"),
    }
}

#[cfg(test)]
mod tests;
