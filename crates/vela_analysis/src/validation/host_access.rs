use std::collections::BTreeSet;

use vela_common::Diagnostic;
use vela_hir::body::{HirAssignOp, HirBody, HirExprKind};
use vela_hir::ids::HirExprId;

use crate::facts::AnalysisFacts;
use crate::registry::{
    RegistryEffectFact, RegistryFacts, RegistryIndexCapabilityFact, RegistryTypeTargetFact,
};
use crate::semantic_facts::{CallTargetFact, MemberTargetFact};
use crate::type_fact::TypeFact;

use super::ExecutableValidationFacts;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostAccessUseKind {
    Read,
    Write,
    Mutate,
    Remove,
    Push,
    Call,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostIndexCapabilityResolutionFact {
    Registered(RegistryIndexCapabilityFact),
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostIndexUseFact {
    pub expression: HirExprId,
    pub receiver: HirExprId,
    pub key: HirExprId,
    pub owner: RegistryTypeTargetFact,
    pub capability: HostIndexCapabilityResolutionFact,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAccessUseFact {
    pub target: HirExprId,
    pub kind: HostAccessUseKind,
    /// Every index segment in root-to-leaf order, including traversal indexes
    /// followed by a field or method.
    pub indexes: Vec<HostIndexUseFact>,
    /// Index whose capability governs this operation. Traversal-only indexes
    /// remain in `indexes` but intentionally preserve the current path policy.
    pub accessed_index: Option<usize>,
    pub effect: RegistryEffectFact,
}

pub(super) fn record_body(
    validation: &mut ExecutableValidationFacts,
    schema: Option<&RegistryFacts>,
    facts: &AnalysisFacts,
    body: &HirBody,
) {
    let Some(schema) = schema else {
        return;
    };
    let mut scaffolding = BTreeSet::new();

    // A receiver prefix is part of one HostTarget plan, not an independent
    // read. Index keys are deliberately not traversed here: they are ordinary
    // evaluated expressions and may contain independent host uses.
    for expression in body.expressions.values() {
        if host_path(body, facts, schema, expression.id).is_some() {
            mark_path_prefixes(body, expression.id, &mut scaffolding);
        }
    }

    for expression in body.expressions.values() {
        match &expression.kind {
            HirExprKind::Assign {
                op: Some(op),
                target: Some(target),
                ..
            } => {
                let Some(path) = host_path(body, facts, schema, *target) else {
                    continue;
                };
                let kind = if *op == HirAssignOp::Set {
                    HostAccessUseKind::Write
                } else {
                    HostAccessUseKind::Mutate
                };
                mark_whole_path(body, *target, &mut scaffolding);
                record_use(
                    validation,
                    body,
                    facts,
                    HostUseRequest {
                        expression: expression.id,
                        target: *target,
                        kind,
                        path,
                        effect: RegistryEffectFact::host_write(),
                    },
                );
            }
            HirExprKind::Call(call) => {
                let Some(field) = body.field(call.callee) else {
                    continue;
                };
                let Some(path) = host_path(body, facts, schema, field.receiver) else {
                    continue;
                };
                let kind = if field.name == "remove"
                    && call.arguments.is_empty()
                    && body.index(field.receiver).is_some()
                {
                    Some(HostAccessUseKind::Remove)
                } else if field.name == "push"
                    && path.segment_count > 0
                    && call.arguments.len() == 1
                {
                    Some(HostAccessUseKind::Push)
                } else if matches!(
                    facts.call_target(expression.id),
                    Some(CallTargetFact::HostMethod { .. })
                ) {
                    Some(HostAccessUseKind::Call)
                } else {
                    None
                };
                let Some(kind) = kind else {
                    continue;
                };
                mark_whole_path(body, field.receiver, &mut scaffolding);
                let effect = if kind == HostAccessUseKind::Call {
                    facts
                        .effect(expression.id)
                        .cloned()
                        .unwrap_or_else(RegistryEffectFact::pure)
                } else {
                    RegistryEffectFact::host_write()
                };
                record_use(
                    validation,
                    body,
                    facts,
                    HostUseRequest {
                        expression: expression.id,
                        target: field.receiver,
                        kind,
                        path,
                        effect,
                    },
                );
            }
            _ => {}
        }
    }

    for expression in body.expressions.values() {
        if scaffolding.contains(&expression.id) {
            continue;
        }
        let Some(path) = host_path(body, facts, schema, expression.id) else {
            continue;
        };
        if path.segment_count == 0 {
            continue;
        }
        record_use(
            validation,
            body,
            facts,
            HostUseRequest {
                expression: expression.id,
                target: expression.id,
                kind: HostAccessUseKind::Read,
                path,
                effect: RegistryEffectFact::host_read(),
            },
        );
    }
}

struct HostUseRequest {
    expression: HirExprId,
    target: HirExprId,
    kind: HostAccessUseKind,
    path: HostPathUse,
    effect: RegistryEffectFact,
}

fn record_use(
    validation: &mut ExecutableValidationFacts,
    body: &HirBody,
    facts: &AnalysisFacts,
    request: HostUseRequest,
) {
    let HostUseRequest {
        expression,
        target,
        kind,
        path,
        effect,
    } = request;
    let accessed_index = (kind != HostAccessUseKind::Call)
        .then(|| body.index(target))
        .flatten()
        .and_then(|target_index| {
            path.indexes
                .iter()
                .position(|index| index.expression == target_index.expression)
        });
    let fact = HostAccessUseFact {
        target,
        kind,
        indexes: path.indexes,
        accessed_index,
        effect,
    };
    if let Some(diagnostic) = host_access_diagnostic(body, facts, expression, &fact) {
        validation.diagnostics.push(diagnostic);
    }
    validation.host_access_uses.insert(expression, fact);
}

fn host_access_diagnostic(
    body: &HirBody,
    facts: &AnalysisFacts,
    expression: HirExprId,
    fact: &HostAccessUseFact,
) -> Option<Diagnostic> {
    let use_span = body.expression(expression)?.origin.span;
    if let Some(index) = fact
        .accessed_index
        .and_then(|index| fact.indexes.get(index))
    {
        return index_diagnostic(body, facts, fact.kind, index, use_span);
    }

    if matches!(
        fact.kind,
        HostAccessUseKind::Write | HostAccessUseKind::Mutate | HostAccessUseKind::Push
    ) && let Some(MemberTargetFact::HostField(field)) = facts.member_target(fact.target)
        && !field.access.writable
        && !field.variant_field
    {
        return Some(
            Diagnostic::error("field is read-only for script writes")
                .with_code("analysis::field_not_writable")
                .with_span(use_span)
                .with_label(use_span, "assignment targets a read-only field")
                .with_label(
                    use_span,
                    "write through an exposed method or a writable field instead",
                ),
        );
    }
    None
}

fn index_diagnostic(
    body: &HirBody,
    facts: &AnalysisFacts,
    kind: HostAccessUseKind,
    index: &HostIndexUseFact,
    use_span: vela_common::Span,
) -> Option<Diagnostic> {
    let owner = &index.owner.name;
    let receiver_span = body.expression(index.receiver)?.origin.span;
    let HostIndexCapabilityResolutionFact::Registered(capability) = &index.capability else {
        return Some(
            Diagnostic::error(format!("type `{owner}` does not support host index access"))
                .with_code("analysis::host_index_not_supported")
                .with_span(use_span)
                .with_label(
                    use_span,
                    "host index access is not registered for this type",
                )
                .with_label(
                    receiver_span,
                    "register a host index capability or expose a field/method instead",
                ),
        );
    };

    let access = index_access(kind)?;
    if !access.allowed(capability) {
        return Some(
            Diagnostic::error(format!(
                "type `{owner}` does not allow host index {}",
                access.name()
            ))
            .with_code(access.code())
            .with_span(use_span)
            .with_label(use_span, access.denial_label())
            .with_label(receiver_span, access.enable_label()),
        );
    }

    let actual = facts.expression(index.key)?;
    if type_is_dynamic(actual) || type_is_dynamic(&capability.key) || actual == &capability.key {
        return None;
    }
    let key_span = body.expression(index.key)?.origin.span;
    Some(
        Diagnostic::error(format!(
            "host index key for `{owner}` must be `{}`",
            capability.key.display_name()
        ))
        .with_code("analysis::host_index_key_mismatch")
        .with_span(use_span)
        .with_label(
            key_span,
            format!("index expression has type `{}`", actual.display_name()),
        ),
    )
}

fn type_is_dynamic(fact: &TypeFact) -> bool {
    matches!(
        fact,
        TypeFact::Unknown | TypeFact::Any | TypeFact::Never | TypeFact::Union(_)
    )
}

#[derive(Clone, Copy)]
enum IndexAccess {
    Read,
    Write,
    Mutate,
    Remove,
}

impl IndexAccess {
    fn allowed(self, capability: &RegistryIndexCapabilityFact) -> bool {
        match self {
            Self::Read => capability.readable,
            Self::Write => capability.writable,
            Self::Mutate => capability.addable,
            Self::Remove => capability.removable,
        }
    }

    const fn code(self) -> &'static str {
        match self {
            Self::Read => "analysis::host_index_not_readable",
            Self::Write => "analysis::host_index_not_writable",
            Self::Mutate => "analysis::host_index_not_mutable",
            Self::Remove => "analysis::host_index_not_removable",
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Read => "reads",
            Self::Write => "writes",
            Self::Mutate => "mutations",
            Self::Remove => "removals",
        }
    }

    const fn denial_label(self) -> &'static str {
        match self {
            Self::Read => "host index capability is not readable",
            Self::Write => "host index capability is not writable",
            Self::Mutate => "host index capability is not addable",
            Self::Remove => "host index capability is not removable",
        }
    }

    const fn enable_label(self) -> &'static str {
        match self {
            Self::Read => "enable readable host index access for this type",
            Self::Write => "enable writable host index access for this type",
            Self::Mutate => "enable addable host index access for this type",
            Self::Remove => "enable removable host index access for this type",
        }
    }
}

const fn index_access(kind: HostAccessUseKind) -> Option<IndexAccess> {
    match kind {
        HostAccessUseKind::Read => Some(IndexAccess::Read),
        HostAccessUseKind::Write | HostAccessUseKind::Push => Some(IndexAccess::Write),
        HostAccessUseKind::Mutate => Some(IndexAccess::Mutate),
        HostAccessUseKind::Remove => Some(IndexAccess::Remove),
        HostAccessUseKind::Call => None,
    }
}

struct HostPathUse {
    indexes: Vec<HostIndexUseFact>,
    segment_count: usize,
}

fn host_path(
    body: &HirBody,
    facts: &AnalysisFacts,
    schema: &RegistryFacts,
    expression: HirExprId,
) -> Option<HostPathUse> {
    match &body.expression(expression)?.kind {
        HirExprKind::Path(_) => {
            let TypeFact::Host { name } = facts.expression(expression)? else {
                return None;
            };
            schema.type_target_fact(name)?;
            Some(HostPathUse {
                indexes: Vec::new(),
                segment_count: 0,
            })
        }
        HirExprKind::Paren {
            expression: Some(inner),
        } => host_path(body, facts, schema, *inner),
        HirExprKind::Field(field) => {
            if matches!(
                facts.member_target(expression),
                Some(MemberTargetFact::HostProperty { .. })
            ) {
                let TypeFact::Host { name } = facts.expression(expression)? else {
                    return None;
                };
                schema.type_target_fact(name)?;
                return Some(HostPathUse {
                    indexes: Vec::new(),
                    segment_count: 0,
                });
            }
            let mut path = host_path(body, facts, schema, field.receiver)?;
            if !matches!(
                facts.member_target(expression),
                Some(MemberTargetFact::HostField(_))
            ) {
                return None;
            }
            path.segment_count += 1;
            Some(path)
        }
        HirExprKind::Index(index) => {
            let mut path = host_path(body, facts, schema, index.receiver)?;
            let TypeFact::Host { name } = facts.expression(index.receiver)? else {
                return None;
            };
            let owner = schema.type_target_fact(name)?.clone();
            let capability = schema.index_capability_fact(name).cloned().map_or(
                HostIndexCapabilityResolutionFact::Missing,
                HostIndexCapabilityResolutionFact::Registered,
            );
            path.indexes.push(HostIndexUseFact {
                expression,
                receiver: index.receiver,
                key: index.index,
                owner,
                capability,
            });
            path.segment_count += 1;
            Some(path)
        }
        _ if matches!(facts.expression(expression), Some(TypeFact::Host { .. })) => {
            let TypeFact::Host { name } = facts.expression(expression)? else {
                return None;
            };
            schema.type_target_fact(name)?;
            Some(HostPathUse {
                indexes: Vec::new(),
                segment_count: 0,
            })
        }
        _ => None,
    }
}

fn mark_path_prefixes(
    body: &HirBody,
    expression: HirExprId,
    scaffolding: &mut BTreeSet<HirExprId>,
) {
    match &body.expression(expression).map(|value| &value.kind) {
        Some(HirExprKind::Paren {
            expression: Some(inner),
        }) => mark_whole_path(body, *inner, scaffolding),
        Some(HirExprKind::Field(field)) => mark_whole_path(body, field.receiver, scaffolding),
        Some(HirExprKind::Index(index)) => mark_whole_path(body, index.receiver, scaffolding),
        _ => {}
    }
}

fn mark_whole_path(body: &HirBody, expression: HirExprId, scaffolding: &mut BTreeSet<HirExprId>) {
    scaffolding.insert(expression);
    mark_path_prefixes(body, expression, scaffolding);
}
