use std::collections::{BTreeMap, BTreeSet};

use vela_common::PrimitiveTag;
use vela_hir::attributes::derived_traits;
use vela_hir::body::{HirBinaryOp, HirBody, HirBodyRoot, HirExprKind, HirScopeKind, HirStmtKind};
use vela_hir::ids::{HirBodyId, HirDeclId, HirExprId};
use vela_hir::module_graph::{DeclarationKind, ModuleGraph};
use vela_hir::type_hint::ImplMetadataKind;

use crate::facts::AnalysisFacts;
use crate::semantic_facts::CallTargetFact;
use crate::type_fact::TypeFact;

use super::{
    ArrayOrderingCapabilityFact, ArrayOrderingMethod, ArrayOrderingValueKind, BuiltinOperatorTrait,
    CapabilityFact, ExecutableValidationFacts, LoopControlFact, LoopControlKind,
    LoopControlPlacement, OperatorCapabilityFact,
};

pub(super) struct CapabilityIndex {
    canonical_declarations: BTreeMap<String, HirDeclId>,
    traits: BTreeMap<HirDeclId, BTreeSet<BuiltinOperatorTrait>>,
}

impl CapabilityIndex {
    pub(super) fn new(graph: &ModuleGraph) -> Self {
        let canonical_declarations = graph
            .declarations()
            .filter(|declaration| {
                matches!(
                    declaration.kind,
                    DeclarationKind::Struct | DeclarationKind::Enum
                )
            })
            .filter_map(|declaration| {
                graph
                    .qualified_declaration_name(declaration.id)
                    .map(|name| (name, declaration.id))
            })
            .collect::<BTreeMap<_, _>>();
        let mut traits = BTreeMap::<_, BTreeSet<_>>::new();

        for declaration in graph
            .declarations()
            .filter(|declaration| declaration.kind == DeclarationKind::Struct)
        {
            for trait_name in derived_traits(graph.declaration_attrs(declaration.id)) {
                if let Some(trait_name) = builtin_trait(&trait_name) {
                    traits.entry(declaration.id).or_default().insert(trait_name);
                }
            }
        }

        for declaration in graph.declarations_by_kind(DeclarationKind::Impl) {
            let Some(metadata) = graph.impl_metadata(declaration.id) else {
                continue;
            };
            let ImplMetadataKind::Trait { trait_path } = &metadata.kind else {
                continue;
            };
            let [trait_name] = trait_path.as_slice() else {
                continue;
            };
            let Some(trait_name) = builtin_trait(trait_name) else {
                continue;
            };
            let Some(target_name) =
                qualified_target_name(graph, declaration.module, metadata.target_path.as_slice())
            else {
                continue;
            };
            let Some(target) = canonical_declarations.get(&target_name) else {
                continue;
            };
            traits.entry(*target).or_default().insert(trait_name);
        }

        Self {
            canonical_declarations,
            traits,
        }
    }

    fn supports(
        &self,
        declaration: HirDeclId,
        required: BuiltinOperatorTrait,
        type_name: String,
    ) -> CapabilityFact {
        if self
            .traits
            .get(&declaration)
            .is_some_and(|traits| traits.contains(&required))
        {
            CapabilityFact::Supported
        } else {
            CapabilityFact::Unsupported { type_name }
        }
    }

    fn declaration_for_fact(&self, fact: &TypeFact) -> Option<HirDeclId> {
        let name = match fact {
            TypeFact::Record { name } | TypeFact::Enum { name, .. } => name,
            _ => return None,
        };
        self.canonical_declarations.get(name).copied()
    }
}

pub(super) fn record_body(
    validation: &mut ExecutableValidationFacts,
    capabilities: &CapabilityIndex,
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    body: &HirBody,
) {
    for expression in body.expressions.values() {
        if let Some(fact) = operator_capability(capabilities, graph, facts, body, expression.id) {
            if let Some(diagnostic) = super::diagnostics::operator(body, expression, &fact) {
                validation.diagnostics.push(diagnostic);
            }
            validation.operators.insert(expression.id, fact);
        }
        if let Some(fact) =
            array_ordering_capability(capabilities, graph, facts, body, expression.id)
        {
            if let Some(diagnostic) = super::diagnostics::array_ordering(expression, &fact) {
                validation.diagnostics.push(diagnostic);
            }
            validation.array_ordering.insert(expression.id, fact);
        }
    }

    for statement in body.statements.values() {
        let kind = match statement.kind {
            HirStmtKind::Break => LoopControlKind::Break,
            HirStmtKind::Continue => LoopControlKind::Continue,
            _ => continue,
        };
        let fact = LoopControlFact {
            kind,
            placement: loop_placement(body, statement.scope),
        };
        if let Some(diagnostic) = super::diagnostics::loop_control(statement, fact) {
            validation.diagnostics.push(diagnostic);
        }
        validation.loop_controls.insert(statement.id, fact);
    }
}

fn operator_capability(
    capabilities: &CapabilityIndex,
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    body: &HirBody,
    expression: HirExprId,
) -> Option<OperatorCapabilityFact> {
    let HirExprKind::Binary {
        op: Some(operator),
        lhs: Some(lhs),
        rhs: Some(rhs),
    } = body.expression(expression)?.kind
    else {
        return None;
    };

    match operator {
        HirBinaryOp::IdentityEqual | HirBinaryOp::IdentityNotEqual => {
            Some(OperatorCapabilityFact::ReferenceIdentity {
                operator,
                lhs_expression: lhs,
                lhs: identity_capability(graph, facts, lhs),
                rhs_expression: rhs,
                rhs: identity_capability(graph, facts, rhs),
            })
        }
        HirBinaryOp::Equal | HirBinaryOp::NotEqual => {
            Some(OperatorCapabilityFact::ComparisonTrait {
                operator,
                receiver: lhs,
                required: BuiltinOperatorTrait::PartialEq,
                capability: comparison_capability(
                    capabilities,
                    graph,
                    facts,
                    lhs,
                    BuiltinOperatorTrait::PartialEq,
                ),
            })
        }
        HirBinaryOp::Less
        | HirBinaryOp::LessEqual
        | HirBinaryOp::Greater
        | HirBinaryOp::GreaterEqual => Some(OperatorCapabilityFact::ComparisonTrait {
            operator,
            receiver: lhs,
            required: BuiltinOperatorTrait::PartialOrd,
            capability: comparison_capability(
                capabilities,
                graph,
                facts,
                lhs,
                BuiltinOperatorTrait::PartialOrd,
            ),
        }),
        HirBinaryOp::Range
        | HirBinaryOp::RangeInclusive
        | HirBinaryOp::Add
        | HirBinaryOp::Sub
        | HirBinaryOp::Mul
        | HirBinaryOp::Div
        | HirBinaryOp::Rem
        | HirBinaryOp::Or
        | HirBinaryOp::And => None,
    }
}

fn identity_capability(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    expression: HirExprId,
) -> CapabilityFact {
    if let Some(target) = facts.script_type(expression)
        && graph
            .declaration(target.declaration)
            .is_some_and(|declaration| {
                matches!(
                    declaration.kind,
                    DeclarationKind::Struct | DeclarationKind::Enum
                )
            })
    {
        return CapabilityFact::Supported;
    }

    match facts.expression(expression) {
        Some(TypeFact::Primitive(_) | TypeFact::Range) => CapabilityFact::Unsupported {
            type_name: facts
                .expression(expression)
                .expect("matched expression fact")
                .display_name(),
        },
        Some(
            TypeFact::Unknown
            | TypeFact::Any
            | TypeFact::Union(_)
            | TypeFact::Module { .. }
            | TypeFact::Trait { .. },
        )
        | None => CapabilityFact::Dynamic,
        Some(
            TypeFact::Never
            | TypeFact::Array { .. }
            | TypeFact::ArrayView { .. }
            | TypeFact::ArrayMut { .. }
            | TypeFact::Map { .. }
            | TypeFact::MapView { .. }
            | TypeFact::MapMut { .. }
            | TypeFact::Set { .. }
            | TypeFact::SetView { .. }
            | TypeFact::SetMut { .. }
            | TypeFact::Iterator { .. }
            | TypeFact::Tuple { .. }
            | TypeFact::Option { .. }
            | TypeFact::OptionSome { .. }
            | TypeFact::OptionNone
            | TypeFact::Result { .. }
            | TypeFact::ResultOk { .. }
            | TypeFact::ResultErr { .. }
            | TypeFact::Function { .. }
            | TypeFact::Closure
            | TypeFact::LogicalRecord(_)
            | TypeFact::Record { .. }
            | TypeFact::Enum { .. }
            | TypeFact::Host { .. },
        ) => CapabilityFact::Supported,
    }
}

fn comparison_capability(
    capabilities: &CapabilityIndex,
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    expression: HirExprId,
    required: BuiltinOperatorTrait,
) -> CapabilityFact {
    if let Some(target) = facts.script_type(expression)
        && graph
            .declaration(target.declaration)
            .is_some_and(|declaration| {
                matches!(
                    declaration.kind,
                    DeclarationKind::Struct | DeclarationKind::Enum
                )
            })
    {
        return capabilities.supports(
            target.declaration,
            required,
            expression_type_name(graph, facts, expression, target.declaration),
        );
    }

    match facts.expression(expression) {
        Some(
            TypeFact::Unknown
            | TypeFact::Any
            | TypeFact::Union(_)
            | TypeFact::Record { .. }
            | TypeFact::Enum { .. }
            | TypeFact::LogicalRecord(_)
            | TypeFact::Host { .. }
            | TypeFact::Trait { .. }
            | TypeFact::Module { .. },
        )
        | None => CapabilityFact::Dynamic,
        Some(
            TypeFact::Never
            | TypeFact::Primitive(_)
            | TypeFact::Range
            | TypeFact::Array { .. }
            | TypeFact::ArrayView { .. }
            | TypeFact::ArrayMut { .. }
            | TypeFact::Map { .. }
            | TypeFact::MapView { .. }
            | TypeFact::MapMut { .. }
            | TypeFact::Set { .. }
            | TypeFact::SetView { .. }
            | TypeFact::SetMut { .. }
            | TypeFact::Iterator { .. }
            | TypeFact::Tuple { .. }
            | TypeFact::Option { .. }
            | TypeFact::OptionSome { .. }
            | TypeFact::OptionNone
            | TypeFact::Result { .. }
            | TypeFact::ResultOk { .. }
            | TypeFact::ResultErr { .. }
            | TypeFact::Function { .. }
            | TypeFact::Closure,
        ) => CapabilityFact::Supported,
    }
}

fn array_ordering_capability(
    capabilities: &CapabilityIndex,
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    body: &HirBody,
    expression: HirExprId,
) -> Option<ArrayOrderingCapabilityFact> {
    let call = body.call(expression)?;
    let field = body.field(call.callee)?;
    let method = match field.name.as_str() {
        "sort" => ArrayOrderingMethod::Sort,
        "sort_by" => ArrayOrderingMethod::SortBy,
        "min" => ArrayOrderingMethod::Min,
        "max" => ArrayOrderingMethod::Max,
        _ => return None,
    };
    if !matches!(
        facts.call_target(expression),
        Some(CallTargetFact::StdlibMethod { name }) if name == &field.name
    ) {
        return None;
    }
    let (TypeFact::Array { element }
    | TypeFact::ArrayView { element }
    | TypeFact::ArrayMut { element, .. }) = facts.expression(field.receiver)?
    else {
        return None;
    };

    let (value_kind, capability) = if method == ArrayOrderingMethod::SortBy {
        let capability = call
            .arguments
            .first()
            .and_then(|argument| argument.value)
            .and_then(|callback| direct_lambda_body(body, callback))
            .and_then(|lambda| graph.body(lambda))
            .and_then(body_value_expression)
            .map_or(CapabilityFact::Dynamic, |value| {
                ord_capability_for_expression(capabilities, graph, facts, value)
            });
        (ArrayOrderingValueKind::Key, capability)
    } else {
        (
            ArrayOrderingValueKind::Element,
            ord_capability_for_fact(capabilities, element),
        )
    };

    Some(ArrayOrderingCapabilityFact {
        method,
        value_kind,
        capability,
    })
}

fn ord_capability_for_expression(
    capabilities: &CapabilityIndex,
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    expression: HirExprId,
) -> CapabilityFact {
    if let Some(target) = facts.script_type(expression) {
        return capabilities.supports(
            target.declaration,
            BuiltinOperatorTrait::Ord,
            expression_type_name(graph, facts, expression, target.declaration),
        );
    }
    facts
        .expression(expression)
        .map_or(CapabilityFact::Dynamic, |fact| {
            ord_capability_for_fact(capabilities, fact)
        })
}

fn ord_capability_for_fact(capabilities: &CapabilityIndex, fact: &TypeFact) -> CapabilityFact {
    if let Some(declaration) = capabilities.declaration_for_fact(fact) {
        return capabilities.supports(declaration, BuiltinOperatorTrait::Ord, fact.display_name());
    }

    match fact {
        TypeFact::Primitive(
            PrimitiveTag::Bool
            | PrimitiveTag::Char
            | PrimitiveTag::I8
            | PrimitiveTag::I16
            | PrimitiveTag::I32
            | PrimitiveTag::I64
            | PrimitiveTag::U8
            | PrimitiveTag::U16
            | PrimitiveTag::U32
            | PrimitiveTag::U64
            | PrimitiveTag::String
            | PrimitiveTag::Bytes,
        )
        | TypeFact::Never => CapabilityFact::Supported,
        TypeFact::Unknown
        | TypeFact::Any
        | TypeFact::Union(_)
        | TypeFact::Record { .. }
        | TypeFact::Enum { .. }
        | TypeFact::Host { .. }
        | TypeFact::Trait { .. }
        | TypeFact::Module { .. }
        | TypeFact::LogicalRecord(_) => CapabilityFact::Dynamic,
        TypeFact::Primitive(_)
        | TypeFact::Range
        | TypeFact::Array { .. }
        | TypeFact::ArrayView { .. }
        | TypeFact::ArrayMut { .. }
        | TypeFact::Map { .. }
        | TypeFact::MapView { .. }
        | TypeFact::MapMut { .. }
        | TypeFact::Set { .. }
        | TypeFact::SetView { .. }
        | TypeFact::SetMut { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::Tuple { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Function { .. }
        | TypeFact::Closure => CapabilityFact::Unsupported {
            type_name: fact.display_name(),
        },
    }
}

fn direct_lambda_body(body: &HirBody, expression: HirExprId) -> Option<HirBodyId> {
    match body.expression(expression)?.kind {
        HirExprKind::Lambda { body } => Some(body),
        _ => None,
    }
}

fn body_value_expression(body: &HirBody) -> Option<HirExprId> {
    match body.root {
        HirBodyRoot::Expr(expression) => Some(expression),
        HirBodyRoot::Block(block) => body
            .blocks
            .get(&block)?
            .statements
            .last()
            .and_then(|statement| body.statements.get(statement))
            .and_then(|statement| match statement.kind {
                HirStmtKind::Expr {
                    expression: Some(expression),
                    terminated: false,
                } => Some(expression),
                _ => None,
            }),
        HirBodyRoot::Empty => None,
    }
}

fn loop_placement(body: &HirBody, mut scope: vela_hir::ids::HirScopeId) -> LoopControlPlacement {
    loop {
        let Some(current) = body.scopes.get(&scope) else {
            return LoopControlPlacement::UnresolvedScope;
        };
        if current.kind == HirScopeKind::For {
            return LoopControlPlacement::InsideLoop;
        }
        let Some(parent) = current.parent else {
            return LoopControlPlacement::OutsideLoop;
        };
        scope = parent;
    }
}

fn expression_type_name(
    graph: &ModuleGraph,
    facts: &AnalysisFacts,
    expression: HirExprId,
    declaration: HirDeclId,
) -> String {
    facts.expression(expression).map_or_else(
        || {
            graph.declaration(declaration).map_or_else(
                || "unknown".to_owned(),
                |declaration| declaration.name.clone(),
            )
        },
        TypeFact::display_name,
    )
}

fn builtin_trait(name: &str) -> Option<BuiltinOperatorTrait> {
    match name {
        "PartialEq" => Some(BuiltinOperatorTrait::PartialEq),
        "PartialOrd" => Some(BuiltinOperatorTrait::PartialOrd),
        "Ord" => Some(BuiltinOperatorTrait::Ord),
        "Eq" => None,
        _ => None,
    }
}

fn qualified_target_name(
    graph: &ModuleGraph,
    module: vela_hir::ids::ModuleId,
    path: &[String],
) -> Option<String> {
    if path.len() != 1 {
        return (!path.is_empty()).then(|| path.join("::"));
    }
    let module = graph.module_path(module)?;
    if module.segments().is_empty() {
        Some(path[0].clone())
    } else {
        Some(format!("{}::{}", module.join(), path[0]))
    }
}
