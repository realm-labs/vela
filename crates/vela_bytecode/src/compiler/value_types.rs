use std::collections::HashMap;

use vela_common::{PrimitiveTag, Span};
use vela_hir::ids::HirLocalId;
use vela_hir::type_hint::HirTypeHint;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RuntimeTypeFact {
    Primitive(PrimitiveTag),
    Standard(StandardRuntimeType),
    Array(Box<RuntimeTypeFact>),
    Map {
        key: Box<RuntimeTypeFact>,
        value: Box<RuntimeTypeFact>,
    },
    Set(Box<RuntimeTypeFact>),
    Iterator(Box<RuntimeTypeFact>),
    Tuple(Vec<RuntimeTypeFact>),
    Option(Box<RuntimeTypeFact>),
    Result {
        ok: Box<RuntimeTypeFact>,
        err: Box<RuntimeTypeFact>,
    },
}

pub(super) type TypeRef = RuntimeTypeFact;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum StaticExprType {
    Exact(TypeRef),
    UnsuffixedIntegerLiteral,
    UnsuffixedFloatLiteral,
    Dynamic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ExpectedTypeOutcome {
    Proven,
    Contextualized(TypeRef),
    RequiresRuntimeGuard(TypeRef),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum TypeContractContext {
    FunctionParameter {
        name: String,
    },
    NativeParameter {
        function: String,
        name: String,
        index: u16,
    },
    TypedLet {
        name: String,
    },
    Field {
        name: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StandardRuntimeType {
    Array,
    Map,
    Set,
    Range,
    Function,
    Closure,
    Iterator,
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

    pub(super) fn array(element: RuntimeTypeFact) -> Self {
        Self::Array(Box::new(element))
    }

    pub(super) fn map(key: RuntimeTypeFact, value: RuntimeTypeFact) -> Self {
        Self::Map {
            key: Box::new(key),
            value: Box::new(value),
        }
    }

    pub(super) fn set(element: RuntimeTypeFact) -> Self {
        Self::Set(Box::new(element))
    }

    pub(super) fn iterator(item: RuntimeTypeFact) -> Self {
        Self::Iterator(Box::new(item))
    }

    pub(super) fn tuple(elements: Vec<RuntimeTypeFact>) -> Self {
        Self::Tuple(elements)
    }

    pub(super) const fn std_type_name(&self) -> &'static str {
        match self {
            Self::Primitive(PrimitiveTag::Unit) => "Unit",
            Self::Primitive(PrimitiveTag::Bool) => "Bool",
            Self::Primitive(PrimitiveTag::Char) => "Char",
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
            Self::Standard(StandardRuntimeType::Iterator) => "Iterator",
            Self::Standard(StandardRuntimeType::Option) => "Option",
            Self::Standard(StandardRuntimeType::Result) => "Result",
            Self::Array(_) => "Array",
            Self::Map { .. } => "Map",
            Self::Set(_) => "Set",
            Self::Iterator(_) => "Iterator",
            Self::Tuple(_) => "Tuple",
            Self::Option(_) => "Option",
            Self::Result { .. } => "Result",
        }
    }

    pub(super) const fn source_type_name(&self) -> &'static str {
        match self {
            Self::Primitive(PrimitiveTag::Unit) => "()",
            Self::Primitive(PrimitiveTag::Bool) => "bool",
            Self::Primitive(PrimitiveTag::Char) => "char",
            Self::Primitive(PrimitiveTag::I8) => "i8",
            Self::Primitive(PrimitiveTag::I16) => "i16",
            Self::Primitive(PrimitiveTag::I32) => "i32",
            Self::Primitive(PrimitiveTag::I64) => "i64",
            Self::Primitive(PrimitiveTag::U8) => "u8",
            Self::Primitive(PrimitiveTag::U16) => "u16",
            Self::Primitive(PrimitiveTag::U32) => "u32",
            Self::Primitive(PrimitiveTag::U64) => "u64",
            Self::Primitive(PrimitiveTag::F32) => "f32",
            Self::Primitive(PrimitiveTag::F64) => "f64",
            Self::Primitive(PrimitiveTag::String) => "String",
            Self::Primitive(PrimitiveTag::Bytes) => "Bytes",
            Self::Standard(StandardRuntimeType::Array) => "Array",
            Self::Standard(StandardRuntimeType::Map) => "Map",
            Self::Standard(StandardRuntimeType::Set) => "Set",
            Self::Standard(StandardRuntimeType::Range) => "Range",
            Self::Standard(StandardRuntimeType::Function) => "Function",
            Self::Standard(StandardRuntimeType::Closure) => "Closure",
            Self::Standard(StandardRuntimeType::Iterator) => "Iterator",
            Self::Standard(StandardRuntimeType::Option) => "Option",
            Self::Standard(StandardRuntimeType::Result) => "Result",
            Self::Array(_) => "Array",
            Self::Map { .. } => "Map",
            Self::Set(_) => "Set",
            Self::Iterator(_) => "Iterator",
            Self::Tuple(_) => "Tuple",
            Self::Option(_) => "Option",
            Self::Result { .. } => "Result",
        }
    }

    pub(super) fn source_type_display(&self) -> String {
        match self {
            Self::Array(element) => format!("Array<{}>", element.source_type_display()),
            Self::Map { key, value } => {
                format!(
                    "Map<{}, {}>",
                    key.source_type_display(),
                    value.source_type_display()
                )
            }
            Self::Set(element) => format!("Set<{}>", element.source_type_display()),
            Self::Iterator(item) => format!("Iterator<{}>", item.source_type_display()),
            Self::Tuple(elements) => {
                let elements = elements
                    .iter()
                    .map(Self::source_type_display)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({elements})")
            }
            Self::Option(payload) => format!("Option<{}>", payload.source_type_display()),
            Self::Result { ok, err } => {
                format!(
                    "Result<{}, {}>",
                    ok.source_type_display(),
                    err.source_type_display()
                )
            }
            _ => self.source_type_name().to_owned(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ValueTypeFlow {
    locals: HashMap<HirLocalId, RuntimeTypeFact>,
    names: HashMap<String, RuntimeTypeFact>,
}

impl ValueTypeFlow {
    pub(super) fn local(&self, local: HirLocalId) -> Option<RuntimeTypeFact> {
        self.locals.get(&local).cloned()
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

pub(super) fn type_hint_value_type(hint: &HirTypeHint) -> Option<RuntimeTypeFact> {
    let [name] = hint.path.as_slice() else {
        return None;
    };
    match name.as_str() {
        "()" if !hint.args.is_empty() => hint
            .args
            .iter()
            .map(type_hint_value_type)
            .collect::<Option<Vec<_>>>()
            .map(RuntimeTypeFact::tuple),
        "()" => Some(RuntimeTypeFact::primitive(PrimitiveTag::Unit)),
        "bool" => Some(RuntimeTypeFact::primitive(PrimitiveTag::Bool)),
        "char" => Some(RuntimeTypeFact::primitive(PrimitiveTag::Char)),
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
        "String" => Some(RuntimeTypeFact::primitive(PrimitiveTag::String)),
        "Bytes" => Some(RuntimeTypeFact::primitive(PrimitiveTag::Bytes)),
        "Array" if hint.args.len() == 1 => {
            type_hint_value_type(&hint.args[0]).map(RuntimeTypeFact::array)
        }
        "Map" if hint.args.len() == 2 => {
            let key = type_hint_value_type(&hint.args[0])?;
            let value = type_hint_value_type(&hint.args[1])?;
            Some(RuntimeTypeFact::map(key, value))
        }
        "Set" if hint.args.len() == 1 => {
            type_hint_value_type(&hint.args[0]).map(RuntimeTypeFact::set)
        }
        "Iterator" if hint.args.len() == 1 => {
            type_hint_value_type(&hint.args[0]).map(RuntimeTypeFact::iterator)
        }
        "Array" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Array)),
        "Map" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Map)),
        "Set" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Set)),
        "Range" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Range)),
        "Function" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Function)),
        "Closure" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Closure)),
        "Iterator" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Iterator)),
        "Option" if hint.args.len() == 1 => type_hint_value_type(&hint.args[0])
            .map(|payload| RuntimeTypeFact::Option(Box::new(payload))),
        "Option" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Option)),
        "Result" if hint.args.len() == 2 => {
            let ok = type_hint_value_type(&hint.args[0])?;
            let err = type_hint_value_type(&hint.args[1])?;
            Some(RuntimeTypeFact::Result {
                ok: Box::new(ok),
                err: Box::new(err),
            })
        }
        "Result" => Some(RuntimeTypeFact::standard(StandardRuntimeType::Result)),
        _ => None,
    }
}

pub(super) fn check_expected_type(
    actual: StaticExprType,
    expected: RuntimeTypeFact,
    span: Span,
    context: TypeContractContext,
) -> super::CompileResult<ExpectedTypeOutcome> {
    match actual {
        StaticExprType::Exact(actual) if actual == expected => Ok(ExpectedTypeOutcome::Proven),
        StaticExprType::Exact(actual) if accepts_erased_or_parameterized(&actual, &expected) => {
            Ok(ExpectedTypeOutcome::Proven)
        }
        StaticExprType::Exact(actual)
            if erased_outer_matches_parameterized(&actual, &expected)
                || parameterized_outer_matches_erased(&actual, &expected) =>
        {
            Ok(ExpectedTypeOutcome::RequiresRuntimeGuard(expected))
        }
        StaticExprType::Exact(actual) => Err(type_contract_mismatch(
            expected,
            ActualContractType::Exact(actual),
            span,
            context,
        )),
        StaticExprType::UnsuffixedIntegerLiteral
            if expected_primitive_tag(&expected).is_some_and(is_integer_tag) =>
        {
            Ok(ExpectedTypeOutcome::Contextualized(expected))
        }
        StaticExprType::UnsuffixedIntegerLiteral => Err(type_contract_mismatch(
            expected,
            ActualContractType::UnsuffixedIntegerLiteral,
            span,
            context,
        )),
        StaticExprType::UnsuffixedFloatLiteral
            if expected_primitive_tag(&expected).is_some_and(is_float_tag) =>
        {
            Ok(ExpectedTypeOutcome::Contextualized(expected))
        }
        StaticExprType::UnsuffixedFloatLiteral => Err(type_contract_mismatch(
            expected,
            ActualContractType::UnsuffixedFloatLiteral,
            span,
            context,
        )),
        StaticExprType::Dynamic => Ok(ExpectedTypeOutcome::RequiresRuntimeGuard(expected)),
    }
}

fn expected_primitive_tag(expected: &RuntimeTypeFact) -> Option<PrimitiveTag> {
    match expected {
        RuntimeTypeFact::Primitive(tag) => Some(*tag),
        RuntimeTypeFact::Standard(_)
        | RuntimeTypeFact::Array(_)
        | RuntimeTypeFact::Map { .. }
        | RuntimeTypeFact::Set(_)
        | RuntimeTypeFact::Iterator(_)
        | RuntimeTypeFact::Tuple(_)
        | RuntimeTypeFact::Option(_)
        | RuntimeTypeFact::Result { .. } => None,
    }
}

fn is_integer_tag(tag: PrimitiveTag) -> bool {
    vela_analysis::literals::NumericLiteralKind::Integer.accepts_primitive(tag)
}

fn is_float_tag(tag: PrimitiveTag) -> bool {
    vela_analysis::literals::NumericLiteralKind::Float.accepts_primitive(tag)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ActualContractType {
    Exact(RuntimeTypeFact),
    UnsuffixedIntegerLiteral,
    UnsuffixedFloatLiteral,
}

fn type_contract_mismatch(
    expected: RuntimeTypeFact,
    actual: ActualContractType,
    span: Span,
    context: TypeContractContext,
) -> super::CompileError {
    super::CompileError::new(super::CompileErrorKind::SemanticDiagnostics(vec![
        vela_common::Diagnostic::error(format!(
            "type contract mismatch for {}",
            context.description()
        ))
        .with_code("compiler::type_contract_mismatch")
        .with_span(span)
        .with_label(
            span,
            format!(
                "expected `{}`, found {}",
                expected.source_type_display(),
                actual.description()
            ),
        ),
    ]))
}

impl TypeContractContext {
    fn description(&self) -> String {
        match self {
            Self::FunctionParameter { name } => format!("parameter `{name}`"),
            Self::NativeParameter { function, name, .. } => {
                format!("native parameter `{function}::{name}`")
            }
            Self::TypedLet { name } => format!("typed local `{name}`"),
            Self::Field { name } => format!("field `{name}`"),
        }
    }
}

impl ActualContractType {
    fn description(&self) -> String {
        match self {
            Self::Exact(actual) => format!("`{}`", actual.source_type_display()),
            Self::UnsuffixedIntegerLiteral => "unsuffixed integer literal".to_owned(),
            Self::UnsuffixedFloatLiteral => "unsuffixed float literal".to_owned(),
        }
    }
}

fn accepts_erased_or_parameterized(actual: &RuntimeTypeFact, expected: &RuntimeTypeFact) -> bool {
    parameterized_outer_matches_erased(actual, expected)
        || matches!(
            (actual, expected),
            (
                RuntimeTypeFact::Standard(StandardRuntimeType::Closure),
                RuntimeTypeFact::Standard(StandardRuntimeType::Function)
            ) | (
                RuntimeTypeFact::Option(_),
                RuntimeTypeFact::Standard(StandardRuntimeType::Option)
            ) | (
                RuntimeTypeFact::Result { .. },
                RuntimeTypeFact::Standard(StandardRuntimeType::Result)
            )
        )
}

fn erased_outer_matches_parameterized(
    actual: &RuntimeTypeFact,
    expected: &RuntimeTypeFact,
) -> bool {
    matches!(
        (actual, expected),
        (
            RuntimeTypeFact::Standard(StandardRuntimeType::Array),
            RuntimeTypeFact::Array(_)
        ) | (
            RuntimeTypeFact::Standard(StandardRuntimeType::Map),
            RuntimeTypeFact::Map { .. }
        ) | (
            RuntimeTypeFact::Standard(StandardRuntimeType::Set),
            RuntimeTypeFact::Set(_)
        ) | (
            RuntimeTypeFact::Standard(StandardRuntimeType::Iterator),
            RuntimeTypeFact::Iterator(_)
        ) | (
            RuntimeTypeFact::Standard(StandardRuntimeType::Option),
            RuntimeTypeFact::Option(_)
        ) | (
            RuntimeTypeFact::Standard(StandardRuntimeType::Result),
            RuntimeTypeFact::Result { .. }
        )
    )
}

fn parameterized_outer_matches_erased(
    actual: &RuntimeTypeFact,
    expected: &RuntimeTypeFact,
) -> bool {
    matches!(
        (actual, expected),
        (
            RuntimeTypeFact::Array(_),
            RuntimeTypeFact::Standard(StandardRuntimeType::Array)
        ) | (
            RuntimeTypeFact::Map { .. },
            RuntimeTypeFact::Standard(StandardRuntimeType::Map)
        ) | (
            RuntimeTypeFact::Set(_),
            RuntimeTypeFact::Standard(StandardRuntimeType::Set)
        ) | (
            RuntimeTypeFact::Iterator(_),
            RuntimeTypeFact::Standard(StandardRuntimeType::Iterator)
        )
    )
}
