use vela_common::Span;
use vela_syntax::ast::{
    Attribute, ConstItem, EnumItem, FunctionItem, GlobalItem, ImplItem, StructItem, TraitItem,
};

use crate::attributes::{HirAttribute, attrs_from_syntax};
use crate::ids::HirNodeId;
use crate::type_hint::{
    ConstMetadata, EnumShape, FunctionSignature, GlobalMetadata, HirTypeHint, ImplMetadata,
    ParamHint, StructFieldHint, StructShape, TraitShape,
};

use super::syntax_summary::SyntaxModuleSummary;

pub(super) fn attrs(
    summary: Option<&SyntaxModuleSummary>,
    index: usize,
    fallback: &[Attribute],
) -> Vec<HirAttribute> {
    summary.map_or_else(
        || attrs_from_syntax(fallback),
        |summary| summary.attrs_or(index, attrs_from_syntax(fallback)),
    )
}

pub(super) fn const_metadata(
    summary: Option<&SyntaxModuleSummary>,
    index: usize,
    item: &ConstItem,
) -> ConstMetadata {
    summary.map_or_else(
        || ConstMetadata::from_syntax(item),
        |summary| summary.const_metadata_or(index, ConstMetadata::from_syntax(item)),
    )
}

pub(super) fn global_metadata(
    summary: Option<&SyntaxModuleSummary>,
    index: usize,
    item: &GlobalItem,
) -> GlobalMetadata {
    summary.map_or_else(
        || GlobalMetadata::from_syntax(item),
        |summary| summary.global_metadata_or(index, GlobalMetadata::from_syntax(item)),
    )
}

pub(super) fn function_signature(
    summary: Option<&SyntaxModuleSummary>,
    index: usize,
    item: &FunctionItem,
) -> FunctionSignature {
    summary.map_or_else(
        || fallback_function_signature(item),
        |summary| summary.function_signature_or(index, fallback_function_signature(item)),
    )
}

pub(super) fn struct_shape(
    summary: Option<&SyntaxModuleSummary>,
    index: usize,
    item: &StructItem,
) -> StructShape {
    let fallback = fallback_struct_shape(item);
    match summary {
        Some(summary) => {
            let shape = summary.struct_shape_or(index, fallback.clone());
            if same_struct_fields(&shape, &fallback) {
                shape
            } else {
                fallback
            }
        }
        None => fallback,
    }
}

pub(super) fn enum_shape(
    summary: Option<&SyntaxModuleSummary>,
    index: usize,
    item: &EnumItem,
) -> EnumShape {
    summary.map_or_else(
        || EnumShape::from_syntax(item),
        |summary| summary.enum_shape_or(index, EnumShape::from_syntax(item)),
    )
}

pub(super) fn trait_shape(
    summary: Option<&SyntaxModuleSummary>,
    index: usize,
    item: &TraitItem,
    default_method_nodes: Vec<Option<(HirNodeId, Span)>>,
) -> TraitShape {
    match summary {
        Some(summary) => summary.trait_shape_or(
            index,
            default_method_nodes.clone(),
            TraitShape::from_syntax(item, default_method_nodes),
        ),
        None => TraitShape::from_syntax(item, default_method_nodes),
    }
}

pub(super) fn impl_metadata(
    summary: Option<&SyntaxModuleSummary>,
    index: usize,
    item: &ImplItem,
    method_nodes: Vec<(HirNodeId, Span)>,
) -> ImplMetadata {
    match summary {
        Some(summary) => summary.impl_metadata_or(
            index,
            method_nodes.clone(),
            ImplMetadata::from_syntax(item, method_nodes),
        ),
        None => ImplMetadata::from_syntax(item, method_nodes),
    }
}

fn fallback_function_signature(item: &FunctionItem) -> FunctionSignature {
    FunctionSignature {
        params: item.params.iter().map(ParamHint::from_syntax).collect(),
        return_type: item.return_type.as_ref().map(HirTypeHint::from_syntax),
    }
}

fn fallback_struct_shape(item: &StructItem) -> StructShape {
    StructShape {
        fields: item
            .fields
            .iter()
            .map(StructFieldHint::from_syntax)
            .collect(),
    }
}

fn same_struct_fields(left: &StructShape, right: &StructShape) -> bool {
    left.fields.len() == right.fields.len()
        && left
            .fields
            .iter()
            .zip(&right.fields)
            .all(|(left, right)| left.name == right.name)
}
