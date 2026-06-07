use vela_common::Span;

use crate::abi::{AccessAbi, EffectAbi};
use crate::report::{HotReloadDiagnostic, HotReloadReport};
use crate::report_detail::HotReloadDiagnosticDetail;

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
    ChangedModules,
    ImpactedModules,
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
        if !report.changed_modules.is_empty() {
            lines.push(HotReloadReportLine::new(
                HotReloadReportLineKind::ChangedModules,
                None,
                None,
                format!("changed modules: {}", report.changed_modules.join(", ")),
            ));
        }
        if !report.impacted_modules.is_empty() {
            lines.push(HotReloadReportLine::new(
                HotReloadReportLineKind::ImpactedModules,
                None,
                None,
                format!("impacted modules: {}", report.impacted_modules.join(", ")),
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
        HotReloadDiagnosticDetail::SchemaMemberAbi { old, new } => format!(
            "schema ABI: old=({}) new=({})",
            render_schema_abi(old),
            render_schema_abi(new)
        ),
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
        HotReloadDiagnosticDetail::TraitMethodAbiList { old, new } => format!(
            "trait method ABI: old=({}) new=({})",
            render_trait_method_abi_list(old),
            render_trait_method_abi_list(new)
        ),
        HotReloadDiagnosticDetail::ModuleExportAbiList { old, new } => format!(
            "module export ABI: old=({}) new=({})",
            render_module_export_abi_list(old),
            render_module_export_abi_list(new)
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

fn render_param_abi_list(params: &[crate::abi::ParamAbi]) -> String {
    if params.is_empty() {
        return "<none>".to_owned();
    }
    params
        .iter()
        .map(render_param_abi)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_param_abi(param: &crate::abi::ParamAbi) -> String {
    let type_hint = param.type_hint.as_deref().unwrap_or("Any");
    if param.has_default {
        format!("{}:{type_hint}=default", param.name)
    } else {
        format!("{}:{type_hint}", param.name)
    }
}

fn render_schema_abi(schema: &crate::schema_abi::SchemaAbi) -> String {
    let kind = schema
        .kind
        .map(crate::schema_abi::SchemaKindAbi::as_str)
        .unwrap_or("unknown");
    format!(
        "kind={kind} hash={} fields=[{}] variants=[{}] traits=[{}]",
        schema.hash,
        render_schema_field_abi_list(&schema.fields),
        render_schema_variant_abi_list(&schema.variants),
        render_schema_trait_impl_abi_list(&schema.trait_impls)
    )
}

fn render_schema_field_abi_list(fields: &[crate::schema_abi::SchemaFieldAbi]) -> String {
    if fields.is_empty() {
        return "<none>".to_owned();
    }
    fields
        .iter()
        .map(render_schema_field_abi)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_schema_field_abi(field: &crate::schema_abi::SchemaFieldAbi) -> String {
    let type_hint = field.type_hint.as_deref().unwrap_or("Any");
    let default = if field.has_default { "=default" } else { "" };
    format!("{}#{}:{type_hint}{default}", field.name, field.id)
}

fn render_schema_variant_abi_list(variants: &[crate::schema_abi::SchemaVariantAbi]) -> String {
    if variants.is_empty() {
        return "<none>".to_owned();
    }
    variants
        .iter()
        .map(render_schema_variant_abi)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_schema_variant_abi(variant: &crate::schema_abi::SchemaVariantAbi) -> String {
    format!(
        "{}#{}({})",
        variant.name,
        variant.id,
        render_schema_field_abi_list(&variant.fields)
    )
}

fn render_schema_trait_impl_abi_list(traits: &[crate::schema_abi::SchemaTraitImplAbi]) -> String {
    if traits.is_empty() {
        return "<none>".to_owned();
    }
    traits
        .iter()
        .map(render_schema_trait_impl_abi)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_schema_trait_impl_abi(trait_impl: &crate::schema_abi::SchemaTraitImplAbi) -> String {
    format!(
        "{}#{}({})",
        trait_impl.name,
        trait_impl.id,
        render_trait_method_abi_list(&trait_impl.methods)
    )
}

fn render_trait_method_abi_list(methods: &[crate::abi::TraitMethodAbi]) -> String {
    if methods.is_empty() {
        return "<none>".to_owned();
    }
    methods
        .iter()
        .map(render_trait_method_abi)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_trait_method_abi(method: &crate::abi::TraitMethodAbi) -> String {
    let params = render_param_abi_list(&method.params);
    let return_type = method.return_type.as_deref().unwrap_or("Any");
    if method.has_default {
        format!(
            "{}#{}({params})->{return_type}=default",
            method.name, method.id
        )
    } else {
        format!("{}#{}({params})->{return_type}", method.name, method.id)
    }
}

fn render_module_export_abi_list(exports: &[crate::module_abi::ModuleExportAbi]) -> String {
    if exports.is_empty() {
        return "<none>".to_owned();
    }
    exports
        .iter()
        .map(render_module_export_abi)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_module_export_abi(export: &crate::module_abi::ModuleExportAbi) -> String {
    let function = export
        .function
        .map(|function| function.to_string())
        .unwrap_or_else(|| "<none>".to_owned());
    format!("{}:{}#{function}", export.name, export.kind.as_str())
}

fn render_effect_abi(effect: &EffectAbi) -> String {
    format!(
        "reads_host={} writes_host={} emits_events={} reads_time={} uses_random={} reads_io={} writes_io={} reads_reflection={} writes_reflection={} calls_reflection={}",
        effect.reads_host,
        effect.writes_host,
        effect.emits_events,
        effect.reads_time,
        effect.uses_random,
        effect.reads_io,
        effect.writes_io,
        effect.reads_reflection,
        effect.writes_reflection,
        effect.calls_reflection
    )
}

fn render_access_abi(access: &AccessAbi) -> String {
    format!(
        "public={} reflective={} callable={}",
        access.public, access.reflective, access.callable
    )
}

fn render_span(span: Span) -> String {
    format!("source {}:{}..{}", span.source.get(), span.start, span.end)
}
