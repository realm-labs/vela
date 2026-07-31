//! Backend-neutral expected-type contract classification.
//!
//! This module owns the semantic decision between a statically proven value,
//! a context-typed numeric literal, a required runtime guard, and a static
//! mismatch. Backends may translate the resulting logical [`TypeFact`] into
//! their physical guard representation, but must not repeat this decision.

use vela_common::{Diagnostic, PrimitiveTag, Span};
use vela_hir::ids::HirExprId;
use vela_hir::module_graph::ModuleGraph;

use crate::literals::{LiteralErrorClass, LiteralResult, NumericLiteralKind, ResolvedLiteralFact};
use crate::type_fact::TypeFact;

pub const TYPE_CONTRACT_MISMATCH_CODE: &str = "compiler::type_contract_mismatch";

/// The statically known shape of a value at an expected-contract boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractActual {
    Exact(TypeFact),
    DeferredNumeric(NumericLiteralKind),
    Dynamic,
}

/// Source-language callable category imposed by an expected contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedCallableKind {
    Function,
    Closure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedCallableKindSet {
    direct_function: bool,
    closure: bool,
}

impl ExpectedCallableKindSet {
    pub const FUNCTION: Self = Self {
        direct_function: true,
        closure: true,
    };
    pub const CLOSURE: Self = Self {
        direct_function: false,
        closure: true,
    };

    #[must_use]
    pub const fn accepts(self, kind: ExpectedCallableKind) -> bool {
        match kind {
            ExpectedCallableKind::Function => self.direct_function,
            ExpectedCallableKind::Closure => self.closure,
        }
    }
}

/// Callable contract semantics that cannot be represented losslessly by a
/// [`TypeFact::Function`] parameter vector.
///
/// In particular, `None` is an erased arity contract and is distinct from a
/// proven zero-argument contract represented by `Some(0)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedCallableContract {
    accepted_kinds: ExpectedCallableKindSet,
    positional_arity: Option<u32>,
}

impl ExpectedCallableContract {
    #[must_use]
    pub const fn new(kind: ExpectedCallableKind, positional_arity: Option<u32>) -> Self {
        Self {
            accepted_kinds: match kind {
                ExpectedCallableKind::Function => ExpectedCallableKindSet::FUNCTION,
                ExpectedCallableKind::Closure => ExpectedCallableKindSet::CLOSURE,
            },
            positional_arity,
        }
    }

    #[must_use]
    pub const fn kind(self) -> ExpectedCallableKind {
        if self.accepted_kinds.direct_function {
            ExpectedCallableKind::Function
        } else {
            ExpectedCallableKind::Closure
        }
    }

    #[must_use]
    pub const fn accepted_kinds(self) -> ExpectedCallableKindSet {
        self.accepted_kinds
    }

    #[must_use]
    pub const fn positional_arity(self) -> Option<u32> {
        self.positional_arity
    }

    fn projected_type_fact(self) -> TypeFact {
        match self.kind() {
            ExpectedCallableKind::Function => TypeFact::function(Vec::new(), TypeFact::Unknown),
            ExpectedCallableKind::Closure => TypeFact::Closure,
        }
    }
}

impl ContractActual {
    /// Converts a validated literal fact into a contract actual.
    ///
    /// An incompatible primitive result intentionally does not duplicate the
    /// HIR literal's suffix/deferred-state metadata. The caller supplies that
    /// HIR-derived actual so a suffixed literal remains an exact primitive
    /// while an unsuffixed literal remains contextual.
    #[must_use]
    pub fn from_literal_result(result: &LiteralResult, incompatible_actual: Self) -> Option<Self> {
        match result {
            Ok(ResolvedLiteralFact::Scalar(value)) => {
                Some(Self::Exact(TypeFact::primitive(value.primitive())))
            }
            Ok(ResolvedLiteralFact::Deferred(value)) => Some(Self::DeferredNumeric(value.kind())),
            Err(error) if error.class() == LiteralErrorClass::IncompatiblePrimitive => {
                Some(incompatible_actual)
            }
            Err(_) => None,
        }
    }

    fn description(&self) -> String {
        match self {
            Self::Exact(actual) => format!("`{}`", contract_type_display(actual)),
            Self::DeferredNumeric(NumericLiteralKind::Integer) => {
                "unsuffixed integer literal".to_owned()
            }
            Self::DeferredNumeric(NumericLiteralKind::Float) => {
                "unsuffixed float literal".to_owned()
            }
            Self::Dynamic => "dynamic value".to_owned(),
        }
    }
}

/// The source-language location imposing an expected contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpectedContractContext {
    FunctionParameter {
        name: String,
    },
    NativeParameter {
        function: String,
        name: String,
        index: u32,
    },
    TypedLet {
        name: String,
    },
    Field {
        name: String,
    },
}

impl ExpectedContractContext {
    #[must_use]
    pub fn description(&self) -> String {
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

/// The semantic result of checking one value against its expected contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpectedContractOutcome {
    Proven,
    Contextualized(TypeFact),
    RequiresRuntimeGuard(TypeFact),
}

/// A successful expected-contract validation keyed by its stable HIR value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirContractValidation {
    expression: HirExprId,
    outcome: ExpectedContractOutcome,
}

impl HirContractValidation {
    #[must_use]
    pub const fn expression(&self) -> HirExprId {
        self.expression
    }

    #[must_use]
    pub const fn outcome(&self) -> &ExpectedContractOutcome {
        &self.outcome
    }

    #[must_use]
    pub fn into_outcome(self) -> ExpectedContractOutcome {
        self.outcome
    }
}

/// A static expected-contract mismatch without a backend error type or span.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractMismatch {
    expected: TypeFact,
    actual: ContractActual,
    context: ExpectedContractContext,
}

impl ContractMismatch {
    #[must_use]
    pub const fn expected(&self) -> &TypeFact {
        &self.expected
    }

    #[must_use]
    pub const fn actual(&self) -> &ContractActual {
        &self.actual
    }

    #[must_use]
    pub const fn context(&self) -> &ExpectedContractContext {
        &self.context
    }

    #[must_use]
    pub fn to_diagnostic(&self, span: Span) -> Diagnostic {
        Diagnostic::error(format!(
            "type contract mismatch for {}",
            self.context.description()
        ))
        .with_code(TYPE_CONTRACT_MISMATCH_CODE)
        .with_span(span)
        .with_label(
            span,
            format!(
                "expected `{}`, found {}",
                contract_type_display(&self.expected),
                self.actual.description()
            ),
        )
    }
}

/// A mismatch attached to the stable HIR expression that supplied the value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirContractMismatch {
    expression: HirExprId,
    mismatch: Box<ContractMismatch>,
}

impl HirContractMismatch {
    #[must_use]
    pub const fn expression(&self) -> HirExprId {
        self.expression
    }

    #[must_use]
    pub const fn mismatch(&self) -> &ContractMismatch {
        &self.mismatch
    }

    #[must_use]
    pub fn to_diagnostic(&self, graph: &ModuleGraph) -> Option<Diagnostic> {
        graph
            .expression_span(self.expression)
            .map(|span| self.mismatch.to_diagnostic(span))
    }
}

/// Classifies a value against an expected contract without backend state.
pub fn check_expected_contract(
    actual: ContractActual,
    expected: TypeFact,
    context: ExpectedContractContext,
) -> Result<ExpectedContractOutcome, Box<ContractMismatch>> {
    if let Some(callable) = erased_callable_contract(&expected) {
        return check_expected_callable_contract_with_projection(
            actual, callable, expected, context,
        );
    }
    if contract_is_erased(&expected) {
        return Ok(ExpectedContractOutcome::Proven);
    }

    match actual {
        ContractActual::Exact(actual) => match contract_relation(&actual, &expected) {
            ContractRelation::Proven => Ok(ExpectedContractOutcome::Proven),
            ContractRelation::RequiresRuntimeGuard => {
                Ok(ExpectedContractOutcome::RequiresRuntimeGuard(expected))
            }
            ContractRelation::Mismatch => Err(Box::new(ContractMismatch {
                expected,
                actual: ContractActual::Exact(actual),
                context,
            })),
        },
        ContractActual::DeferredNumeric(kind)
            if expected_primitive(&expected).is_some_and(|tag| kind.accepts_primitive(tag)) =>
        {
            Ok(ExpectedContractOutcome::Contextualized(expected))
        }
        ContractActual::DeferredNumeric(kind) => Err(Box::new(ContractMismatch {
            expected,
            actual: ContractActual::DeferredNumeric(kind),
            context,
        })),
        ContractActual::Dynamic => Ok(ExpectedContractOutcome::RequiresRuntimeGuard(expected)),
    }
}

/// Classifies a callable value while preserving erased versus exact arity.
///
/// This is the semantic entrypoint for MIR or another backend-neutral caller
/// that already owns a callable kind and optional arity. Backends retain their
/// original physical guard contract; the returned [`TypeFact`] remains only
/// the frozen diagnostic/outcome projection.
pub fn check_expected_callable_contract(
    actual: ContractActual,
    expected: ExpectedCallableContract,
    context: ExpectedContractContext,
) -> Result<ExpectedContractOutcome, Box<ContractMismatch>> {
    let projected = expected.projected_type_fact();
    check_expected_callable_contract_with_projection(actual, expected, projected, context)
}

/// Callable contract validation keyed by the stable HIR value supplying it.
pub fn check_expected_callable_contract_at(
    expression: HirExprId,
    actual: ContractActual,
    expected: ExpectedCallableContract,
    context: ExpectedContractContext,
) -> Result<HirContractValidation, HirContractMismatch> {
    check_expected_callable_contract(actual, expected, context)
        .map(|outcome| HirContractValidation {
            expression,
            outcome,
        })
        .map_err(|mismatch| HirContractMismatch {
            expression,
            mismatch,
        })
}

/// Classifies a value and preserves the stable HIR diagnostic origin.
pub fn check_expected_contract_at(
    expression: HirExprId,
    actual: ContractActual,
    expected: TypeFact,
    context: ExpectedContractContext,
) -> Result<HirContractValidation, HirContractMismatch> {
    check_expected_contract(actual, expected, context)
        .map(|outcome| HirContractValidation {
            expression,
            outcome,
        })
        .map_err(|mismatch| HirContractMismatch {
            expression,
            mismatch,
        })
}

fn expected_primitive(expected: &TypeFact) -> Option<PrimitiveTag> {
    match expected {
        TypeFact::Primitive(tag) => Some(*tag),
        _ => None,
    }
}

fn contract_is_erased(contract: &TypeFact) -> bool {
    matches!(contract, TypeFact::Unknown | TypeFact::Any)
}

fn erased_callable_contract(expected: &TypeFact) -> Option<ExpectedCallableContract> {
    match expected {
        TypeFact::Function { params, returns }
            if params.is_empty() && contract_is_erased(returns) =>
        {
            Some(ExpectedCallableContract::new(
                ExpectedCallableKind::Function,
                None,
            ))
        }
        TypeFact::Closure => Some(ExpectedCallableContract::new(
            ExpectedCallableKind::Closure,
            None,
        )),
        _ => None,
    }
}

fn check_expected_callable_contract_with_projection(
    actual: ContractActual,
    expected: ExpectedCallableContract,
    projected: TypeFact,
    context: ExpectedContractContext,
) -> Result<ExpectedContractOutcome, Box<ContractMismatch>> {
    match actual {
        ContractActual::Exact(TypeFact::Never) => Ok(ExpectedContractOutcome::Proven),
        ContractActual::Exact(actual) if fact_requires_runtime_proof(&actual) => {
            Ok(ExpectedContractOutcome::RequiresRuntimeGuard(projected))
        }
        ContractActual::Exact(actual) => {
            let Some((actual_kind, actual_arity)) = callable_fact(&actual) else {
                return Err(contract_mismatch(
                    projected,
                    ContractActual::Exact(actual),
                    context,
                ));
            };
            let kind_matches = expected.accepted_kinds.accepts(actual_kind);
            if !kind_matches {
                return Err(contract_mismatch(
                    projected,
                    ContractActual::Exact(actual),
                    context,
                ));
            }
            match (actual_arity, expected.positional_arity) {
                (_, None) => Ok(ExpectedContractOutcome::Proven),
                (Some(actual), Some(expected)) if actual == expected => {
                    Ok(ExpectedContractOutcome::Proven)
                }
                (None, Some(_)) => Ok(ExpectedContractOutcome::RequiresRuntimeGuard(projected)),
                (Some(_), Some(_)) => Err(contract_mismatch(
                    projected,
                    ContractActual::Exact(actual),
                    context,
                )),
            }
        }
        ContractActual::DeferredNumeric(kind) => Err(contract_mismatch(
            projected,
            ContractActual::DeferredNumeric(kind),
            context,
        )),
        ContractActual::Dynamic => Ok(ExpectedContractOutcome::RequiresRuntimeGuard(projected)),
    }
}

fn callable_fact(fact: &TypeFact) -> Option<(ExpectedCallableKind, Option<u32>)> {
    match fact {
        TypeFact::Function { params, returns } => {
            let arity = if params.is_empty() && contract_is_erased(returns) {
                None
            } else {
                u32::try_from(params.len()).ok()
            };
            Some((ExpectedCallableKind::Function, arity))
        }
        TypeFact::Closure => Some((ExpectedCallableKind::Closure, None)),
        _ => None,
    }
}

fn contract_mismatch(
    expected: TypeFact,
    actual: ContractActual,
    context: ExpectedContractContext,
) -> Box<ContractMismatch> {
    Box::new(ContractMismatch {
        expected,
        actual,
        context,
    })
}

fn fact_requires_runtime_proof(actual: &TypeFact) -> bool {
    matches!(
        actual,
        TypeFact::Unknown | TypeFact::Any | TypeFact::Union(_)
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContractRelation {
    Proven,
    RequiresRuntimeGuard,
    Mismatch,
}

fn contract_relation(actual: &TypeFact, expected: &TypeFact) -> ContractRelation {
    if contract_is_erased(expected) || actual == expected || matches!(actual, TypeFact::Never) {
        return ContractRelation::Proven;
    }
    if fact_requires_runtime_proof(actual) {
        return ContractRelation::RequiresRuntimeGuard;
    }

    match (actual, expected) {
        (TypeFact::Array { element: actual }, TypeFact::Array { element: expected })
        | (TypeFact::ArrayView { element: actual }, TypeFact::ArrayView { element: expected })
        | (TypeFact::Set { element: actual }, TypeFact::Set { element: expected })
        | (TypeFact::SetView { element: actual }, TypeFact::SetView { element: expected })
        | (TypeFact::Iterator { item: actual }, TypeFact::Iterator { item: expected })
        | (TypeFact::ScopedIterator { item: actual }, TypeFact::Iterator { item: expected })
        | (
            TypeFact::ScopedIterator { item: actual },
            TypeFact::ScopedIterator { item: expected },
        ) => contract_relation(actual, expected),
        (
            TypeFact::ArrayMut {
                element: actual,
                mutation: actual_mutation,
            },
            TypeFact::ArrayMut {
                element: expected,
                mutation: expected_mutation,
            },
        )
        | (
            TypeFact::SetMut {
                element: actual,
                mutation: actual_mutation,
            },
            TypeFact::SetMut {
                element: expected,
                mutation: expected_mutation,
            },
        ) if actual_mutation == expected_mutation => contract_relation(actual, expected),
        (
            TypeFact::Map {
                key: actual_key,
                value: actual_value,
            },
            TypeFact::Map {
                key: expected_key,
                value: expected_value,
            },
        ) => combine_contract_relations([
            contract_relation(actual_key, expected_key),
            contract_relation(actual_value, expected_value),
        ]),
        (
            TypeFact::MapView {
                key: actual_key,
                value: actual_value,
            },
            TypeFact::MapView {
                key: expected_key,
                value: expected_value,
            },
        ) => combine_contract_relations([
            contract_relation(actual_key, expected_key),
            contract_relation(actual_value, expected_value),
        ]),
        (
            TypeFact::MapMut {
                key: actual_key,
                value: actual_value,
                mutation: actual_mutation,
            },
            TypeFact::MapMut {
                key: expected_key,
                value: expected_value,
                mutation: expected_mutation,
            },
        ) if actual_mutation == expected_mutation => combine_contract_relations([
            contract_relation(actual_key, expected_key),
            contract_relation(actual_value, expected_value),
        ]),
        (TypeFact::Tuple { elements: actual }, TypeFact::Tuple { elements: expected })
            if actual.len() == expected.len() =>
        {
            combine_contract_relations(
                actual
                    .iter()
                    .zip(expected)
                    .map(|(actual, expected)| contract_relation(actual, expected)),
            )
        }
        (TypeFact::Option { some: actual }, TypeFact::Option { some: expected })
        | (TypeFact::OptionSome { some: actual }, TypeFact::Option { some: expected })
        | (TypeFact::OptionSome { some: actual }, TypeFact::OptionSome { some: expected }) => {
            contract_relation(actual, expected)
        }
        (TypeFact::OptionNone, TypeFact::Option { .. }) => ContractRelation::Proven,
        (
            TypeFact::Result {
                ok: actual_ok,
                err: actual_err,
            },
            TypeFact::Result {
                ok: expected_ok,
                err: expected_err,
            },
        ) => combine_contract_relations([
            contract_relation(actual_ok, expected_ok),
            contract_relation(actual_err, expected_err),
        ]),
        (TypeFact::ResultOk { ok: actual }, TypeFact::Result { ok: expected, .. })
        | (TypeFact::ResultOk { ok: actual }, TypeFact::ResultOk { ok: expected }) => {
            contract_relation(actual, expected)
        }
        (TypeFact::ResultErr { err: actual }, TypeFact::Result { err: expected, .. })
        | (TypeFact::ResultErr { err: actual }, TypeFact::ResultErr { err: expected }) => {
            contract_relation(actual, expected)
        }
        (
            TypeFact::Enum {
                name: actual_name,
                variant: actual_variant,
            },
            TypeFact::Enum {
                name: expected_name,
                variant: expected_variant,
            },
        ) if actual_name == expected_name => match (actual_variant, expected_variant) {
            (_, None) => ContractRelation::Proven,
            (None, Some(_)) => ContractRelation::RequiresRuntimeGuard,
            (Some(actual), Some(expected)) if actual == expected => ContractRelation::Proven,
            (Some(_), Some(_)) => ContractRelation::Mismatch,
        },
        _ => ContractRelation::Mismatch,
    }
}

fn combine_contract_relations(
    relations: impl IntoIterator<Item = ContractRelation>,
) -> ContractRelation {
    relations
        .into_iter()
        .fold(ContractRelation::Proven, |combined, relation| {
            match (combined, relation) {
                (ContractRelation::Mismatch, _) | (_, ContractRelation::Mismatch) => {
                    ContractRelation::Mismatch
                }
                (ContractRelation::RequiresRuntimeGuard, _)
                | (_, ContractRelation::RequiresRuntimeGuard) => {
                    ContractRelation::RequiresRuntimeGuard
                }
                (ContractRelation::Proven, ContractRelation::Proven) => ContractRelation::Proven,
            }
        })
}

fn contract_type_display(contract: &TypeFact) -> String {
    match contract {
        TypeFact::Unknown => "unknown".to_owned(),
        TypeFact::Never => "never".to_owned(),
        TypeFact::Any => "Any".to_owned(),
        TypeFact::Primitive(PrimitiveTag::String) => "String".to_owned(),
        TypeFact::Primitive(PrimitiveTag::Bytes) => "Bytes".to_owned(),
        TypeFact::Primitive(tag) => tag.name().to_owned(),
        TypeFact::Range => "Range".to_owned(),
        TypeFact::Array { element } if contract_is_erased(element) => "Array".to_owned(),
        TypeFact::Array { element } => format!("Array<{}>", contract_type_display(element)),
        TypeFact::ArrayView { element } => {
            format!("ArrayView<{}>", contract_type_display(element))
        }
        TypeFact::ArrayMut { element, mutation } => format!(
            "ArrayMut<{}> ({})",
            contract_type_display(element),
            mutation.as_str()
        ),
        TypeFact::Map { key, value } if contract_is_erased(key) && contract_is_erased(value) => {
            "Map".to_owned()
        }
        TypeFact::Map { key, value } => format!(
            "Map<{}, {}>",
            contract_type_display(key),
            contract_type_display(value)
        ),
        TypeFact::MapView { key, value } => format!(
            "MapView<{}, {}>",
            contract_type_display(key),
            contract_type_display(value)
        ),
        TypeFact::MapMut {
            key,
            value,
            mutation,
        } => format!(
            "MapMut<{}, {}> ({})",
            contract_type_display(key),
            contract_type_display(value),
            mutation.as_str()
        ),
        TypeFact::Set { element } if contract_is_erased(element) => "Set".to_owned(),
        TypeFact::Set { element } => format!("Set<{}>", contract_type_display(element)),
        TypeFact::SetView { element } => {
            format!("SetView<{}>", contract_type_display(element))
        }
        TypeFact::SetMut { element, mutation } => format!(
            "SetMut<{}> ({})",
            contract_type_display(element),
            mutation.as_str()
        ),
        TypeFact::Iterator { item } if contract_is_erased(item) => "Iterator".to_owned(),
        TypeFact::Iterator { item } => format!("Iterator<{}>", contract_type_display(item)),
        TypeFact::ScopedIterator { item } if contract_is_erased(item) => {
            "ScopedIterator".to_owned()
        }
        TypeFact::ScopedIterator { item } => {
            format!("ScopedIterator<{}>", contract_type_display(item))
        }
        TypeFact::Tuple { elements } => format!(
            "({})",
            elements
                .iter()
                .map(contract_type_display)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeFact::Option { some } if contract_is_erased(some) => "Option".to_owned(),
        TypeFact::Option { some } => format!("Option<{}>", contract_type_display(some)),
        TypeFact::OptionSome { some } => format!("Option::Some<{}>", contract_type_display(some)),
        TypeFact::OptionNone => "Option::None".to_owned(),
        TypeFact::Result { ok, err } if contract_is_erased(ok) && contract_is_erased(err) => {
            "Result".to_owned()
        }
        TypeFact::Result { ok, err } => format!(
            "Result<{}, {}>",
            contract_type_display(ok),
            contract_type_display(err)
        ),
        TypeFact::ResultOk { ok } => format!("Result::Ok<{}>", contract_type_display(ok)),
        TypeFact::ResultErr { err } => format!("Result::Err<{}>", contract_type_display(err)),
        TypeFact::Function { .. } => "Function".to_owned(),
        TypeFact::Closure => "Closure".to_owned(),
        TypeFact::LogicalRecord(record) => record.runtime_name().to_owned(),
        TypeFact::Record { name }
        | TypeFact::Host { name }
        | TypeFact::Trait { name }
        | TypeFact::Module { name } => name.clone(),
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => format!("{name}::{variant}"),
        TypeFact::Enum {
            name,
            variant: None,
        } => name.clone(),
        TypeFact::Union(facts) => facts
            .iter()
            .map(contract_type_display)
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

#[cfg(test)]
mod tests;
