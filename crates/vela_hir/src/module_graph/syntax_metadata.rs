use vela_common::{Diagnostic, Span};

use crate::attributes::HirAttribute;
use crate::ids::HirNodeId;
use crate::type_hint::{
    ConstMetadata, EnumShape, FunctionSignature, GlobalMetadata, ImplMetadata, StructShape,
    TraitShape,
};

use super::syntax_summary::SyntaxModuleSummary;

pub(super) fn attrs(summary: &SyntaxModuleSummary, index: usize) -> Vec<HirAttribute> {
    summary.attrs_or(index, Vec::new())
}

pub(super) fn const_metadata(summary: &SyntaxModuleSummary, index: usize) -> ConstMetadata {
    summary.const_metadata_or(
        index,
        ConstMetadata {
            type_hint: None,
            value_span: summary.module_span(),
        },
    )
}

pub(super) fn const_initializer_diagnostics(
    summary: &SyntaxModuleSummary,
    index: usize,
) -> Vec<Diagnostic> {
    summary
        .const_initializer_diagnostics(index)
        .unwrap_or_default()
}

pub(super) fn global_metadata(
    summary: &SyntaxModuleSummary,
    index: usize,
) -> Option<GlobalMetadata> {
    summary.global_metadata(index)
}

pub(super) fn function_signature(summary: &SyntaxModuleSummary, index: usize) -> FunctionSignature {
    summary.function_signature_or(
        index,
        FunctionSignature {
            params: Vec::new(),
            return_type: None,
        },
    )
}

pub(super) fn struct_shape(summary: &SyntaxModuleSummary, index: usize) -> StructShape {
    summary.struct_shape_or(index, StructShape { fields: Vec::new() })
}

pub(super) fn enum_shape(summary: &SyntaxModuleSummary, index: usize) -> EnumShape {
    summary.enum_shape_or(
        index,
        EnumShape {
            variants: Vec::new(),
        },
    )
}

pub(super) fn trait_shape(
    summary: &SyntaxModuleSummary,
    index: usize,
    default_method_nodes: Vec<Option<(HirNodeId, Span)>>,
) -> TraitShape {
    summary.trait_shape_or(
        index,
        default_method_nodes,
        TraitShape {
            methods: Vec::new(),
        },
    )
}

pub(super) fn impl_metadata(
    summary: &SyntaxModuleSummary,
    index: usize,
    method_nodes: Vec<(HirNodeId, Span)>,
) -> ImplMetadata {
    summary.impl_metadata_or(
        index,
        method_nodes,
        ImplMetadata {
            kind: crate::type_hint::ImplMetadataKind::Inherent,
            target_path: Vec::new(),
            methods: Vec::new(),
        },
    )
}
