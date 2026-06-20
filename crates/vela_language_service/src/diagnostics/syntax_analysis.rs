use std::collections::{BTreeMap, BTreeSet};

use vela_analysis::{
    completion::{
        CompletionKind as AnalysisCompletionKind, member_completions as analysis_member_completions,
    },
    registry::{
        RegistryEffectFact, RegistryFacts, RegistryFieldAccessFact, RegistryMethodAccessFact,
    },
    stdlib::stdlib_method_fact,
    type_fact::TypeFact,
};
use vela_common::{Diagnostic, PrimitiveTag, SourceId, Span};
use vela_hir::{
    ids::ModuleId,
    module_graph::{Declaration, DeclarationKind, ModuleGraph},
};
use vela_syntax::ast::{
    AstNode, SyntaxMatchExpr, SyntaxPattern, SyntaxPatternKind, SyntaxRecordExpr, SyntaxSourceFile,
};
use vela_syntax::{Parse as SyntaxParse, TextRange as SyntaxTextRange};

use crate::{TextRange, expression_facts, member_access};

pub(super) fn source_diagnostics(
    parsed: &SyntaxParse<SyntaxSourceFile>,
    source: SourceId,
    graph: &ModuleGraph,
    module: Option<ModuleId>,
    facts: &RegistryFacts,
) -> Vec<Diagnostic> {
    let expression_facts = expression_facts::collect(graph, parsed, facts);
    let method_sites = member_access::member_call_sites(parsed);
    let method_ranges = method_sites
        .iter()
        .map(|site| (site.member_range.start, site.member_range.end))
        .collect::<BTreeSet<_>>();
    let mut diagnostics = method_sites
        .iter()
        .filter_map(|site| {
            member_site_diagnostic(
                source,
                facts,
                &expression_facts,
                site.receiver_range,
                site.member_range,
                &site.member,
                AnalysisCompletionKind::Method,
            )
        })
        .collect::<Vec<_>>();

    diagnostics.extend(
        member_access::member_access_sites(parsed)
            .into_iter()
            .filter(|site| {
                !method_ranges.contains(&(site.member_range.start, site.member_range.end))
            })
            .filter_map(|site| {
                member_site_diagnostic(
                    source,
                    facts,
                    &expression_facts,
                    site.receiver_range,
                    site.member_range,
                    &site.member,
                    AnalysisCompletionKind::Field,
                )
            }),
    );
    diagnostics.extend(match_exhaustiveness_diagnostics(
        parsed,
        source,
        &expression_facts,
        facts,
    ));
    if let Some(module) = module {
        diagnostics.extend(record_constructor_diagnostics(
            parsed, source, graph, module,
        ));
    }
    diagnostics
}

fn record_constructor_diagnostics(
    parsed: &SyntaxParse<SyntaxSourceFile>,
    source: SourceId,
    graph: &ModuleGraph,
    module: ModuleId,
) -> Vec<Diagnostic> {
    parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxRecordExpr::cast)
        .flat_map(|expr| {
            let path = expr.path_segments();
            let Some(declaration) = record_constructor_declaration(graph, module, &path) else {
                return Vec::new();
            };
            let Some(shape) = graph.struct_shape(declaration.id) else {
                return Vec::new();
            };
            let explicit = expr
                .fields()
                .into_iter()
                .filter_map(|field| field.label_text())
                .collect::<BTreeSet<_>>();
            let span = syntax_span(source, expr.syntax().text_range());
            missing_required_fields(shape, &explicit)
                .into_iter()
                .map(|field| missing_field_diagnostic(&declaration.name, &field.name, span))
                .collect()
        })
        .collect()
}

fn record_constructor_declaration<'a>(
    graph: &'a ModuleGraph,
    current_module: ModuleId,
    path: &[String],
) -> Option<&'a Declaration> {
    let name = path.last()?;
    graph.declarations().find(|declaration| {
        declaration.kind == DeclarationKind::Struct
            && declaration.name == *name
            && declaration_path_matches(graph, current_module, declaration, path)
    })
}

fn declaration_path_matches(
    graph: &ModuleGraph,
    current_module: ModuleId,
    declaration: &Declaration,
    path: &[String],
) -> bool {
    let Some(module_path) = graph.module_path(declaration.module) else {
        return false;
    };
    if path.len() == 1 {
        return declaration.module == current_module;
    }
    let expected = path[..path.len().saturating_sub(1)].join("::");
    module_path.join() == expected
}

fn missing_required_fields<'a>(
    shape: &'a vela_hir::type_hint::StructShape,
    explicit: &BTreeSet<String>,
) -> Vec<&'a vela_hir::type_hint::StructFieldHint> {
    shape
        .fields
        .iter()
        .filter(|field| field.default_value_span.is_none())
        .filter(|field| !explicit.contains(&field.name))
        .collect()
}

fn missing_field_diagnostic(type_name: &str, field: &str, span: Span) -> Diagnostic {
    Diagnostic::error(format!(
        "missing constructor field `{field}` for `{type_name}`"
    ))
    .with_code("analysis::missing_constructor_field")
    .with_span(span)
    .with_label(span, "required field is not provided and has no default")
}

fn match_exhaustiveness_diagnostics(
    parsed: &SyntaxParse<SyntaxSourceFile>,
    source: SourceId,
    expression_facts: &BTreeMap<(usize, usize), TypeFact>,
    facts: &RegistryFacts,
) -> Vec<Diagnostic> {
    parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(SyntaxMatchExpr::cast)
        .filter_map(|expr| {
            let scrutinee = expr.scrutinee()?;
            let scrutinee_range = text_range_key(scrutinee.syntax().text_range());
            let enum_shape = enum_shape(expression_facts.get(&scrutinee_range)?, facts)?;
            if enum_shape.variants.is_empty() || match_has_catch_all(&expr) {
                return None;
            }

            let covered = expr
                .arms()
                .into_iter()
                .filter(|arm| arm.guard().is_none())
                .filter_map(|arm| {
                    let pattern = arm.pattern()?;
                    pattern_variant_name(&pattern)
                })
                .collect::<BTreeSet<_>>();
            let missing = enum_shape
                .variants
                .into_iter()
                .filter(|variant| !covered.contains(variant))
                .collect::<Vec<_>>();
            if missing.is_empty() {
                return None;
            }

            let span = syntax_span(source, expr.syntax().text_range());
            let mut diagnostic = Diagnostic::warning(format!(
                "match on `{}` does not cover all known variants",
                enum_shape.name
            ))
            .with_code("analysis::non_exhaustive_match")
            .with_span(span)
            .with_label(span, format!("missing variants: {}", missing.join(", ")));
            if !expr.arms().iter().any(|arm| arm.guard().is_none()) {
                diagnostic = diagnostic.with_label(
                    span,
                    "guarded arms do not make a match exhaustive for diagnostics",
                );
            }
            Some(diagnostic)
        })
        .collect()
}

struct EnumShape {
    name: String,
    variants: Vec<String>,
}

fn enum_shape(scrutinee_fact: &TypeFact, facts: &RegistryFacts) -> Option<EnumShape> {
    match scrutinee_fact {
        TypeFact::Enum {
            name,
            variant: None,
        } => Some(EnumShape {
            name: name.clone(),
            variants: facts.variant_names(name),
        }),
        TypeFact::Option { .. } | TypeFact::OptionSome { .. } | TypeFact::OptionNone => {
            Some(EnumShape {
                name: "Option".to_owned(),
                variants: vec!["Some".to_owned(), "None".to_owned()],
            })
        }
        TypeFact::Result { .. } | TypeFact::ResultOk { .. } | TypeFact::ResultErr { .. } => {
            Some(EnumShape {
                name: "Result".to_owned(),
                variants: vec!["Ok".to_owned(), "Err".to_owned()],
            })
        }
        _ => None,
    }
}

fn match_has_catch_all(expr: &SyntaxMatchExpr) -> bool {
    expr.arms().iter().any(|arm| {
        arm.guard().is_none()
            && arm.pattern().is_some_and(|pattern| {
                matches!(
                    pattern.pattern_kind(),
                    Some(SyntaxPatternKind::Wildcard | SyntaxPatternKind::Binding)
                )
            })
    })
}

fn pattern_variant_name(pattern: &SyntaxPattern) -> Option<String> {
    match pattern.pattern_kind()? {
        SyntaxPatternKind::Path => pattern.path_segments().last().cloned(),
        SyntaxPatternKind::TupleVariant => {
            pattern.as_tuple_variant()?.path_segments().last().cloned()
        }
        SyntaxPatternKind::RecordVariant => {
            pattern.as_record_variant()?.path_segments().last().cloned()
        }
        SyntaxPatternKind::Wildcard | SyntaxPatternKind::Literal | SyntaxPatternKind::Binding => {
            None
        }
    }
}

fn syntax_span(source: SourceId, range: SyntaxTextRange) -> Span {
    Span::new(source, u32::from(range.start()), u32::from(range.end()))
}

fn text_range_key(range: SyntaxTextRange) -> (usize, usize) {
    (
        u32::from(range.start()) as usize,
        u32::from(range.end()) as usize,
    )
}

fn member_site_diagnostic(
    source: SourceId,
    facts: &RegistryFacts,
    expression_facts: &BTreeMap<(usize, usize), TypeFact>,
    receiver_range: TextRange,
    member_range: TextRange,
    member: &str,
    kind: AnalysisCompletionKind,
) -> Option<Diagnostic> {
    let receiver = expression_facts.get(&(receiver_range.start, receiver_range.end))?;
    if !is_precise_receiver(facts, receiver) || member_exists(facts, receiver, member, kind) {
        return None;
    }

    let span = Span::new(source, member_range.start as u32, member_range.end as u32);
    let candidates = ranked_member_candidates(facts, receiver, member, kind);
    let extra_labels = match kind {
        AnalysisCompletionKind::Field => {
            field_candidate_access_labels(facts, receiver, &candidates)
        }
        AnalysisCompletionKind::Method => {
            method_candidate_access_labels(facts, receiver, &candidates)
        }
        _ => Vec::new(),
    };
    Some(unknown_member_diagnostic(
        unknown_member_code(kind),
        format!(
            "unknown {} `{member}` for `{}`",
            member_kind_name(kind),
            receiver.display_name()
        ),
        span,
        candidates,
        extra_labels,
    ))
}

fn is_precise_receiver(facts: &RegistryFacts, receiver: &TypeFact) -> bool {
    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } | TypeFact::Trait { name } => {
            facts.type_fact(name).is_some() || facts.trait_fact(name).is_some()
        }
        TypeFact::Enum {
            name,
            variant: Some(_),
        } => facts.type_fact(name).is_some(),
        TypeFact::Array { .. }
        | TypeFact::Map { .. }
        | TypeFact::Set { .. }
        | TypeFact::Iterator { .. }
        | TypeFact::Option { .. }
        | TypeFact::OptionSome { .. }
        | TypeFact::OptionNone
        | TypeFact::Result { .. }
        | TypeFact::ResultOk { .. }
        | TypeFact::ResultErr { .. }
        | TypeFact::Range
        | TypeFact::Primitive(PrimitiveTag::String)
        | TypeFact::Primitive(PrimitiveTag::Bytes)
        | TypeFact::Primitive(PrimitiveTag::Char) => true,
        _ => false,
    }
}

fn member_exists(
    facts: &RegistryFacts,
    receiver: &TypeFact,
    member: &str,
    kind: AnalysisCompletionKind,
) -> bool {
    match kind {
        AnalysisCompletionKind::Field => field_exists(facts, receiver, member),
        AnalysisCompletionKind::Method => method_exists(facts, receiver, member),
        _ => false,
    }
}

fn field_exists(facts: &RegistryFacts, receiver: &TypeFact, field: &str) -> bool {
    field_owner(receiver)
        .as_deref()
        .and_then(|owner| facts.field_fact(owner, field))
        .is_some()
}

fn method_exists(facts: &RegistryFacts, receiver: &TypeFact, method: &str) -> bool {
    if stdlib_method_fact(receiver, method, None).is_some() {
        return true;
    }

    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } => {
            facts.method_fact(name, method).is_some()
        }
        TypeFact::Trait { name } => facts.trait_method_fact(name, method).is_some(),
        _ => false,
    }
}

fn field_owner(receiver: &TypeFact) -> Option<String> {
    match receiver {
        TypeFact::Host { name } | TypeFact::Record { name } => Some(name.clone()),
        TypeFact::Enum {
            name,
            variant: Some(variant),
        } => Some(format!("{name}::{variant}")),
        _ => None,
    }
}

fn ranked_member_candidates(
    facts: &RegistryFacts,
    receiver: &TypeFact,
    member: &str,
    kind: AnalysisCompletionKind,
) -> Vec<String> {
    let mut candidates = analysis_member_completions(facts, receiver)
        .into_iter()
        .filter(|completion| completion.kind == kind)
        .map(|completion| (edit_distance(member, &completion.label), completion.label))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    candidates
        .into_iter()
        .take(3)
        .map(|(_, candidate)| candidate)
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    for (left_index, left_char) in left.chars().enumerate() {
        let mut current = Vec::with_capacity(right_chars.len() + 1);
        current.push(left_index + 1);
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let substitution_cost = usize::from(left_char != *right_char);
            current.push(
                (previous[right_index + 1] + 1)
                    .min(current[right_index] + 1)
                    .min(previous[right_index] + substitution_cost),
            );
        }
        previous = current;
    }
    previous[right_chars.len()]
}

fn field_candidate_access_labels(
    facts: &RegistryFacts,
    receiver: &TypeFact,
    candidates: &[String],
) -> Vec<String> {
    let TypeFact::Host { name: owner } = receiver else {
        return Vec::new();
    };
    candidates
        .iter()
        .filter_map(|candidate| {
            facts
                .field_access_fact(owner, candidate)
                .map(field_access_label)
        })
        .collect()
}

fn field_access_label(access: &RegistryFieldAccessFact) -> String {
    let read_hint = if access.readable {
        "readable"
    } else {
        "not script-readable"
    };
    let write_hint = if access.writable {
        "writable"
    } else {
        "read-only"
    };
    let mut label = format!(
        "candidate `{}.{}` is {read_hint} and {write_hint}",
        access.owner, access.name
    );
    if !access.required_permissions.is_empty() {
        label.push_str(&format!(
            "; requires permission {}",
            access.required_permissions.join(", ")
        ));
    }
    label
}

fn method_candidate_access_labels(
    facts: &RegistryFacts,
    receiver: &TypeFact,
    candidates: &[String],
) -> Vec<String> {
    let TypeFact::Host { name: owner } = receiver else {
        return Vec::new();
    };
    candidates
        .iter()
        .filter_map(|candidate| {
            let access = facts.method_access_fact(owner, candidate)?;
            let effects = facts.method_effect_fact(owner, candidate);
            Some(method_access_label(access, effects))
        })
        .collect()
}

fn method_access_label(
    access: &RegistryMethodAccessFact,
    effects: Option<&RegistryEffectFact>,
) -> String {
    let callable_hint = if access.reflect_callable {
        "reflect-callable"
    } else {
        "not reflect-callable"
    };
    let visibility_hint = if access.public { "public" } else { "private" };
    let effect_hint =
        effects.map_or_else(|| "unknown".to_owned(), RegistryEffectFact::display_name);
    let mut label = format!(
        "candidate `{}.{}` is {visibility_hint} and {callable_hint} with effects {effect_hint}",
        access.owner, access.name
    );
    if !access.required_permissions.is_empty() {
        label.push_str(&format!(
            "; requires permission {}",
            access.required_permissions.join(", ")
        ));
    }
    label
}

fn unknown_member_diagnostic(
    code: &'static str,
    message: String,
    span: Span,
    candidates: Vec<String>,
    extra_labels: Vec<String>,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(message)
        .with_code(code)
        .with_span(span)
        .with_label(span, "unknown member access");
    for candidate in &candidates {
        diagnostic = diagnostic.with_candidate(candidate);
    }
    if let Some(candidate) = candidates.first() {
        diagnostic = diagnostic.with_label(span, format!("did you mean `{candidate}`?"));
    }
    if candidates.len() > 1 {
        diagnostic = diagnostic.with_label(
            span,
            format!("similar candidates: {}", candidates.join(", ")),
        );
    }
    for label in extra_labels {
        diagnostic = diagnostic.with_label(span, label);
    }
    diagnostic
}

const fn unknown_member_code(kind: AnalysisCompletionKind) -> &'static str {
    match kind {
        AnalysisCompletionKind::Field => "analysis::unknown_field",
        AnalysisCompletionKind::Method => "analysis::unknown_method",
        _ => "analysis::unknown_member",
    }
}

const fn member_kind_name(kind: AnalysisCompletionKind) -> &'static str {
    match kind {
        AnalysisCompletionKind::Field => "field",
        AnalysisCompletionKind::Method => "method",
        _ => "member",
    }
}
