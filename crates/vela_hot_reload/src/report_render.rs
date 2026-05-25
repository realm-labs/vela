use vela_common::Span;

use crate::{
    AccessAbi, EffectAbi, HotReloadDiagnostic, HotReloadDiagnosticDetail, HotReloadReport,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HotReloadReportLine {
    pub kind: HotReloadReportLineKind,
    pub diagnostic_index: Option<usize>,
    pub span: Option<Span>,
    pub text: String,
}

impl HotReloadReportLine {
    #[must_use]
    pub fn new(
        kind: HotReloadReportLineKind,
        diagnostic_index: Option<usize>,
        span: Option<Span>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            diagnostic_index,
            span,
            text: text.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HotReloadReportLineKind {
    Summary,
    ChangedFunctions,
    Diagnostic,
    Detail,
    RepairHint,
    SourceDiagnostic,
    SourceLabel,
}

#[must_use]
pub(crate) fn render_lines(report: &HotReloadReport) -> Vec<HotReloadReportLine> {
    let mut lines = Vec::new();
    if report.accepted {
        let transition = match report.to_version {
            Some(to_version) => format!("v{} -> v{}", report.from_version.0, to_version.0),
            None => format!("v{}", report.from_version.0),
        };
        lines.push(HotReloadReportLine::new(
            HotReloadReportLineKind::Summary,
            None,
            None,
            format!("hot reload accepted: {transition}"),
        ));
        if !report.changed_functions.is_empty() {
            lines.push(HotReloadReportLine::new(
                HotReloadReportLineKind::ChangedFunctions,
                None,
                None,
                format!("changed functions: {}", report.changed_functions.join(", ")),
            ));
        }
        return lines;
    }

    lines.push(HotReloadReportLine::new(
        HotReloadReportLineKind::Summary,
        None,
        None,
        format!("hot reload rejected: v{} unchanged", report.from_version.0),
    ));
    for (index, diagnostic) in report.errors.iter().enumerate() {
        push_diagnostic_lines(&mut lines, index, diagnostic);
    }
    lines
}

fn push_diagnostic_lines(
    lines: &mut Vec<HotReloadReportLine>,
    index: usize,
    diagnostic: &HotReloadDiagnostic,
) {
    let target = diagnostic
        .target
        .as_ref()
        .map(|target| format!("{target}: "))
        .unwrap_or_default();
    lines.push(HotReloadReportLine::new(
        HotReloadReportLineKind::Diagnostic,
        Some(index),
        diagnostic.source_span,
        format!("[{}] {target}{}", diagnostic.code, diagnostic.reason),
    ));

    if let Some(detail) = &diagnostic.detail {
        lines.push(HotReloadReportLine::new(
            HotReloadReportLineKind::Detail,
            Some(index),
            None,
            render_detail(detail),
        ));
    }

    if let Some(repair_hint) = &diagnostic.repair_hint {
        lines.push(HotReloadReportLine::new(
            HotReloadReportLineKind::RepairHint,
            Some(index),
            None,
            format!("repair: {repair_hint}"),
        ));
    }

    for source_diagnostic in &diagnostic.source_diagnostics {
        let code = source_diagnostic
            .code
            .as_deref()
            .map(|code| format!(" [{code}]"))
            .unwrap_or_default();
        lines.push(HotReloadReportLine::new(
            HotReloadReportLineKind::SourceDiagnostic,
            Some(index),
            source_diagnostic.span,
            format!(
                "source {}{}: {}",
                source_diagnostic.severity, code, source_diagnostic.message
            ),
        ));
    }

    for label in &diagnostic.labels {
        lines.push(HotReloadReportLine::new(
            HotReloadReportLineKind::SourceLabel,
            Some(index),
            Some(label.span),
            format!("label {}: {}", render_span(label.span), label.message),
        ));
    }
}

fn render_detail(detail: &HotReloadDiagnosticDetail) -> String {
    match detail {
        HotReloadDiagnosticDetail::FunctionParameterList { old, new } => format!(
            "parameters: old=({}) new=({})",
            render_list(old),
            render_list(new)
        ),
        HotReloadDiagnosticDetail::FunctionParameterAbiList { old, new } => format!(
            "parameter ABI: old=({}) new=({})",
            render_param_abi_list(old),
            render_param_abi_list(new)
        ),
        HotReloadDiagnosticDetail::FunctionReturnAbi { old, new } => format!(
            "function return ABI: old={} new={}",
            render_optional(old),
            render_optional(new)
        ),
        HotReloadDiagnosticDetail::AddedFunctionParameters { added } => {
            format!("added required parameters: {}", render_list(added))
        }
        HotReloadDiagnosticDetail::SchemaHash { old_hash, new_hash } => {
            let new_hash = new_hash
                .map(|hash| hash.to_string())
                .unwrap_or_else(|| "removed".to_owned());
            format!("schema hash: old={old_hash} new={new_hash}")
        }
        HotReloadDiagnosticDetail::FunctionEventAbi { old, new } => format!(
            "function event: old={} new={}",
            render_optional(old),
            render_optional(new)
        ),
        HotReloadDiagnosticDetail::FunctionEffectAbi { old, new } => format!(
            "function effects: old=({}) new=({})",
            render_effect_abi(old),
            render_effect_abi(new)
        ),
        HotReloadDiagnosticDetail::FunctionAccessAbi { old, new } => format!(
            "function access: old=({}) new=({})",
            render_access_abi(old),
            render_access_abi(new)
        ),
        HotReloadDiagnosticDetail::MethodEffectAbi { old, new } => format!(
            "method effects: old=({}) new=({})",
            render_effect_abi(old),
            render_effect_abi(new)
        ),
        HotReloadDiagnosticDetail::MethodParameterAbiList { old, new } => format!(
            "method parameter ABI: old=({}) new=({})",
            render_param_abi_list(old),
            render_param_abi_list(new)
        ),
        HotReloadDiagnosticDetail::MethodReturnAbi { old, new } => format!(
            "method return ABI: old={} new={}",
            render_optional(old),
            render_optional(new)
        ),
        HotReloadDiagnosticDetail::MethodAccessAbi { old, new } => format!(
            "method access: old=({}) new=({})",
            render_access_abi(old),
            render_access_abi(new)
        ),
    }
}

fn render_optional(item: &Option<String>) -> &str {
    item.as_deref().unwrap_or("<none>")
}

fn render_list(items: &[String]) -> String {
    if items.is_empty() {
        "<none>".to_owned()
    } else {
        items.join(", ")
    }
}

fn render_param_abi_list(params: &[crate::ParamAbi]) -> String {
    if params.is_empty() {
        return "<none>".to_owned();
    }
    params
        .iter()
        .map(render_param_abi)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_param_abi(param: &crate::ParamAbi) -> String {
    let type_hint = param.type_hint.as_deref().unwrap_or("Any");
    if param.has_default {
        format!("{}:{type_hint}=default", param.name)
    } else {
        format!("{}:{type_hint}", param.name)
    }
}

fn render_effect_abi(effect: &EffectAbi) -> String {
    format!(
        "reads_host={} writes_host={} emits_events={}",
        effect.reads_host, effect.writes_host, effect.emits_events
    )
}

fn render_access_abi(access: &AccessAbi) -> String {
    format!(
        "public={} reflective={} permissions=[{}]",
        access.public,
        access.reflective,
        access.required_permissions.join(", ")
    )
}

fn render_span(span: Span) -> String {
    format!("source {}:{}..{}", span.source.get(), span.start, span.end)
}
