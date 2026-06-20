use vela_common::{Diagnostic, SourceId, Span};
use vela_syntax::ast::{
    AstChildren, AstNode, SyntaxAttribute, SyntaxConstItem, SyntaxEnumItem, SyntaxEnumVariant,
    SyntaxFunctionItem, SyntaxGlobalItem, SyntaxImplItem, SyntaxImplMethod, SyntaxItem,
    SyntaxParam, SyntaxParamList, SyntaxSourceFile, SyntaxStructField, SyntaxStructItem,
    SyntaxTraitItem, SyntaxTraitMethod, SyntaxTypeHint, SyntaxUseItem, Visibility,
};
use vela_syntax::{Parse as SyntaxParse, SyntaxKind, TextRange};

use crate::attributes::HirAttribute;
use crate::ids::HirNodeId;
use crate::top_level::validate_syntax_const_initializer;
use crate::type_hint::{
    ConstMetadata, EnumShape, EnumVariantFieldsHint, EnumVariantHint, FunctionSignature,
    GlobalMetadata, HirTypeHint, ImplMetadata, ImplMetadataKind, ImplMethodMetadata, ParamHint,
    StructFieldHint, StructShape, TraitMethodMetadata, TraitShape,
};

use super::model::DeclarationKind;
use super::names::{inherent_impl_declaration_name, trait_impl_declaration_name};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SyntaxModuleSummary {
    source: SourceId,
    module_span: Span,
    items: Vec<SyntaxItem>,
    item_headers: Vec<SyntaxItemHeader>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SyntaxBodySourceParts {
    pub(super) default_params: Vec<SyntaxParam>,
    pub(super) body: vela_syntax::ast::SyntaxBlock,
}

impl SyntaxBodySourceParts {
    pub(super) fn body_span(&self, source: SourceId) -> Span {
        span_for(source, self.body.syntax().text_range())
    }
}

impl SyntaxModuleSummary {
    pub(super) fn from_parse(source: SourceId, parsed: &SyntaxParse<SyntaxSourceFile>) -> Self {
        let (items, item_headers): (Vec<_>, Vec<_>) = parsed
            .tree()
            .items()
            .filter_map(|item| {
                let header = SyntaxItemHeader::from_item(source, &item)?;
                Some((item, header))
            })
            .unzip();
        let module_span = item_headers
            .first()
            .map_or_else(|| Span::new(source, 0, 0), SyntaxItemHeader::span);
        Self {
            source,
            module_span,
            items,
            item_headers,
        }
    }

    pub(super) const fn module_span(&self) -> Span {
        self.module_span
    }

    pub(super) fn items(&self) -> impl Iterator<Item = (usize, SyntaxKind)> + '_ {
        self.items
            .iter()
            .enumerate()
            .map(|(index, item)| (index, item.syntax().kind()))
    }

    pub(super) fn import(&self, index: usize) -> Option<(Vec<String>, Option<String>, Span)> {
        match self.item_headers.get(index) {
            Some(SyntaxItemHeader::Import { path, alias, span }) => {
                Some((path.clone(), alias.clone(), *span))
            }
            _ => None,
        }
    }

    pub(super) fn declaration(
        &self,
        index: usize,
        kind: DeclarationKind,
    ) -> Option<(String, Visibility, Span)> {
        match self.item_headers.get(index) {
            Some(SyntaxItemHeader::Declaration {
                kind: header_kind,
                name,
                visibility,
                span,
            }) if *header_kind == kind => Some((name.clone(), visibility.clone(), *span)),
            _ => None,
        }
    }

    pub(super) fn attrs_or(&self, index: usize, fallback: Vec<HirAttribute>) -> Vec<HirAttribute> {
        self.items
            .get(index)
            .map(|item| attrs_from_cst(self.source, item.attributes()))
            .unwrap_or(fallback)
    }

    pub(super) fn const_metadata_or(&self, index: usize, fallback: ConstMetadata) -> ConstMetadata {
        self.item(index, SyntaxKind::ConstItem)
            .and_then(|item| SyntaxConstItem::cast(item.syntax().clone()))
            .map_or(fallback, |item| const_metadata(self.source, &item))
    }

    pub(super) fn const_initializer_diagnostics(&self, index: usize) -> Option<Vec<Diagnostic>> {
        self.item(index, SyntaxKind::ConstItem)
            .and_then(|item| SyntaxConstItem::cast(item.syntax().clone()))
            .map(|item| validate_syntax_const_initializer(self.source, &item))
    }

    pub(super) fn global_metadata(&self, index: usize) -> Option<GlobalMetadata> {
        self.item(index, SyntaxKind::GlobalItem)
            .and_then(|item| SyntaxGlobalItem::cast(item.syntax().clone()))
            .and_then(|item| global_metadata(self.source, &item))
    }

    pub(super) fn function_signature_or(
        &self,
        index: usize,
        fallback: FunctionSignature,
    ) -> FunctionSignature {
        self.item(index, SyntaxKind::FunctionItem)
            .and_then(|item| SyntaxFunctionItem::cast(item.syntax().clone()))
            .map_or(fallback, |item| {
                function_signature(self.source, item.param_list(), item.return_type())
            })
    }

    pub(super) fn function_body_source(&self, index: usize) -> Option<SyntaxBodySourceParts> {
        self.item(index, SyntaxKind::FunctionItem)
            .and_then(|item| SyntaxFunctionItem::cast(item.syntax().clone()))
            .and_then(|item| body_source(item.param_list(), item.body()))
    }

    pub(super) fn struct_shape_or(&self, index: usize, fallback: StructShape) -> StructShape {
        self.item(index, SyntaxKind::StructItem)
            .and_then(|item| SyntaxStructItem::cast(item.syntax().clone()))
            .map_or(fallback, |item| struct_shape(self.source, &item))
    }

    pub(super) fn enum_shape_or(&self, index: usize, fallback: EnumShape) -> EnumShape {
        self.item(index, SyntaxKind::EnumItem)
            .and_then(|item| SyntaxEnumItem::cast(item.syntax().clone()))
            .map_or(fallback, |item| enum_shape(self.source, &item))
    }

    pub(super) fn trait_shape_or(
        &self,
        index: usize,
        default_method_nodes: Vec<Option<(HirNodeId, Span)>>,
        fallback: TraitShape,
    ) -> TraitShape {
        self.item(index, SyntaxKind::TraitItem)
            .and_then(|item| SyntaxTraitItem::cast(item.syntax().clone()))
            .map_or(fallback, |item| {
                trait_shape(self.source, &item, default_method_nodes)
            })
    }

    pub(super) fn trait_default_body_sources(
        &self,
        index: usize,
    ) -> Vec<Option<SyntaxBodySourceParts>> {
        self.item(index, SyntaxKind::TraitItem)
            .and_then(|item| SyntaxTraitItem::cast(item.syntax().clone()))
            .map(|item| {
                item.methods()
                    .map(|method| body_source(method.param_list(), method.body()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn impl_metadata_or(
        &self,
        index: usize,
        method_nodes: Vec<(HirNodeId, Span)>,
        fallback: ImplMetadata,
    ) -> ImplMetadata {
        self.item(index, SyntaxKind::ImplItem)
            .and_then(|item| SyntaxImplItem::cast(item.syntax().clone()))
            .map_or(fallback, |item| {
                impl_metadata(self.source, &item, method_nodes)
            })
    }

    pub(super) fn impl_method_body_sources(&self, index: usize) -> Vec<SyntaxBodySourceParts> {
        self.item(index, SyntaxKind::ImplItem)
            .and_then(|item| SyntaxImplItem::cast(item.syntax().clone()))
            .map(|item| {
                item.methods()
                    .filter_map(|method| body_source(method.param_list(), method.body()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn item(&self, index: usize, kind: SyntaxKind) -> Option<&SyntaxItem> {
        self.items
            .get(index)
            .filter(|item| item.syntax().kind() == kind)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SyntaxItemHeader {
    Import {
        path: Vec<String>,
        alias: Option<String>,
        span: Span,
    },
    Declaration {
        kind: DeclarationKind,
        name: String,
        visibility: Visibility,
        span: Span,
    },
}

impl SyntaxItemHeader {
    fn from_item(source: SourceId, item: &SyntaxItem) -> Option<Self> {
        match item.syntax().kind() {
            SyntaxKind::UseItem => {
                let use_item = SyntaxUseItem::cast(item.syntax().clone())?;
                Some(Self::Import {
                    path: use_item
                        .path()
                        .map(|path| path.path_segments())
                        .unwrap_or_default(),
                    alias: use_item.alias_text(),
                    span: span_for(source, item.text_range()),
                })
            }
            SyntaxKind::ConstItem => {
                let item = SyntaxConstItem::cast(item.syntax().clone())?;
                declaration_header(
                    source,
                    item.syntax().text_range(),
                    DeclarationKind::Const,
                    item.name_text(),
                    item.is_public(),
                )
            }
            SyntaxKind::GlobalItem => {
                let item = SyntaxGlobalItem::cast(item.syntax().clone())?;
                declaration_header(
                    source,
                    item.syntax().text_range(),
                    DeclarationKind::Global,
                    item.name_text(),
                    item.is_public(),
                )
            }
            SyntaxKind::FunctionItem => {
                let item = SyntaxFunctionItem::cast(item.syntax().clone())?;
                declaration_header(
                    source,
                    item.syntax().text_range(),
                    DeclarationKind::Function,
                    item.name_text(),
                    item.is_public(),
                )
            }
            SyntaxKind::StructItem => {
                let item = SyntaxStructItem::cast(item.syntax().clone())?;
                declaration_header(
                    source,
                    item.syntax().text_range(),
                    DeclarationKind::Struct,
                    item.name_text(),
                    item.is_public(),
                )
            }
            SyntaxKind::EnumItem => {
                let item = SyntaxEnumItem::cast(item.syntax().clone())?;
                declaration_header(
                    source,
                    item.syntax().text_range(),
                    DeclarationKind::Enum,
                    item.name_text(),
                    item.is_public(),
                )
            }
            SyntaxKind::TraitItem => {
                let item = SyntaxTraitItem::cast(item.syntax().clone())?;
                declaration_header(
                    source,
                    item.syntax().text_range(),
                    DeclarationKind::Trait,
                    item.name_text(),
                    item.is_public(),
                )
            }
            SyntaxKind::ImplItem => {
                let item = SyntaxImplItem::cast(item.syntax().clone())?;
                let target_path = item.target_path_segments();
                let trait_path = item.trait_path_segments();
                let name = if trait_path.is_empty() {
                    inherent_impl_declaration_name(&target_path)
                } else {
                    trait_impl_declaration_name(&trait_path, &target_path)
                };
                Some(Self::Declaration {
                    kind: DeclarationKind::Impl,
                    name,
                    visibility: visibility(item.is_public()),
                    span: span_for(source, item.syntax().text_range()),
                })
            }
            _ => None,
        }
    }

    const fn span(&self) -> Span {
        match self {
            Self::Import { span, .. } | Self::Declaration { span, .. } => *span,
        }
    }
}

fn const_metadata(source: SourceId, item: &SyntaxConstItem) -> ConstMetadata {
    ConstMetadata {
        type_hint: item
            .type_hint()
            .as_ref()
            .map(|hint| hir_type_hint(source, hint)),
        value_span: item.value().as_ref().map_or_else(
            || span_for(source, item.syntax().text_range()),
            |value| span_for(source, value.syntax().text_range()),
        ),
    }
}

fn global_metadata(source: SourceId, item: &SyntaxGlobalItem) -> Option<GlobalMetadata> {
    Some(GlobalMetadata {
        type_hint: hir_type_hint(source, &item.type_hint()?),
    })
}

fn function_signature(
    source: SourceId,
    params: Option<SyntaxParamList>,
    return_type: Option<SyntaxTypeHint>,
) -> FunctionSignature {
    FunctionSignature {
        params: params
            .into_iter()
            .flat_map(|params| params.params())
            .filter_map(|param| param_hint(source, &param))
            .collect(),
        return_type: return_type
            .as_ref()
            .map(|return_type| hir_type_hint(source, return_type)),
    }
}

fn body_source(
    params: Option<SyntaxParamList>,
    body: Option<vela_syntax::ast::SyntaxBlock>,
) -> Option<SyntaxBodySourceParts> {
    Some(SyntaxBodySourceParts {
        default_params: params
            .into_iter()
            .flat_map(|params| params.params())
            .collect(),
        body: body?,
    })
}

fn struct_shape(source: SourceId, item: &SyntaxStructItem) -> StructShape {
    StructShape {
        fields: item
            .field_list()
            .into_iter()
            .flat_map(|list| list.fields())
            .filter_map(|field| struct_field_hint(source, &field))
            .collect(),
    }
}

fn enum_shape(source: SourceId, item: &SyntaxEnumItem) -> EnumShape {
    EnumShape {
        variants: item
            .variant_list()
            .into_iter()
            .flat_map(|list| list.variants())
            .filter_map(|variant| enum_variant_hint(source, &variant))
            .collect(),
    }
}

fn trait_shape(
    source: SourceId,
    item: &SyntaxTraitItem,
    default_method_nodes: Vec<Option<(HirNodeId, Span)>>,
) -> TraitShape {
    TraitShape {
        methods: item
            .methods()
            .zip(default_method_nodes)
            .filter_map(|(method, default_body)| {
                trait_method_metadata(source, &method, default_body)
            })
            .collect(),
    }
}

fn impl_metadata(
    source: SourceId,
    item: &SyntaxImplItem,
    method_nodes: Vec<(HirNodeId, Span)>,
) -> ImplMetadata {
    let trait_path = item.trait_path_segments();
    ImplMetadata {
        kind: if trait_path.is_empty() {
            ImplMetadataKind::Inherent
        } else {
            ImplMetadataKind::Trait { trait_path }
        },
        target_path: item.target_path_segments(),
        methods: item
            .methods()
            .zip(method_nodes)
            .filter_map(|(method, (node, span))| impl_method_metadata(source, &method, node, span))
            .collect(),
    }
}

fn trait_method_metadata(
    source: SourceId,
    method: &SyntaxTraitMethod,
    default_body: Option<(HirNodeId, Span)>,
) -> Option<TraitMethodMetadata> {
    let (default_body_node, default_body_span) =
        default_body.map_or((None, None), |(node, span)| (Some(node), Some(span)));
    Some(TraitMethodMetadata {
        attrs: attrs_from_cst(source, method.attributes()),
        name: method.name_text()?,
        span: span_for(source, method.syntax().text_range()),
        signature: function_signature(source, method.param_list(), method.return_type()),
        has_default: method.body().is_some(),
        default_body_node,
        default_body_span,
    })
}

fn impl_method_metadata(
    source: SourceId,
    method: &SyntaxImplMethod,
    node: HirNodeId,
    span: Span,
) -> Option<ImplMethodMetadata> {
    Some(ImplMethodMetadata {
        node,
        name: method.name_text()?,
        signature: function_signature(source, method.param_list(), method.return_type()),
        span,
    })
}

fn enum_variant_hint(source: SourceId, variant: &SyntaxEnumVariant) -> Option<EnumVariantHint> {
    let fields = if let Some(fields) = variant.tuple_field_list() {
        EnumVariantFieldsHint::Tuple(
            fields
                .params()
                .filter_map(|param| param_hint(source, &param))
                .collect(),
        )
    } else if let Some(fields) = variant.record_field_list() {
        EnumVariantFieldsHint::Record(
            fields
                .fields()
                .filter_map(|field| struct_field_hint(source, &field))
                .collect(),
        )
    } else {
        EnumVariantFieldsHint::Unit
    };
    Some(EnumVariantHint {
        attrs: attrs_from_cst(source, variant.attributes()),
        name: variant.name_text()?,
        span: span_for(source, variant.syntax().text_range()),
        fields,
    })
}

fn struct_field_hint(source: SourceId, field: &SyntaxStructField) -> Option<StructFieldHint> {
    Some(StructFieldHint {
        attrs: attrs_from_cst(source, field.attributes()),
        name: field.name_text()?,
        span: span_for(source, field.syntax().text_range()),
        type_hint: field
            .type_hint()
            .as_ref()
            .map(|hint| hir_type_hint(source, hint)),
        default_value_span: field
            .default_value()
            .as_ref()
            .map(|value| span_for(source, value.syntax().text_range())),
    })
}

fn param_hint(source: SourceId, param: &SyntaxParam) -> Option<ParamHint> {
    Some(ParamHint {
        name: param.name_text()?,
        span: span_for(source, param.syntax().text_range()),
        type_hint: param
            .type_hint()
            .as_ref()
            .map(|hint| hir_type_hint(source, hint)),
        default_value_span: param
            .default_value()
            .as_ref()
            .map(|value| span_for(source, value.syntax().text_range())),
    })
}

fn hir_type_hint(source: SourceId, hint: &SyntaxTypeHint) -> HirTypeHint {
    HirTypeHint {
        path: hint.path_segments(),
        args: hint
            .type_arg_list()
            .into_iter()
            .flat_map(|args| args.type_hints())
            .map(|arg| hir_type_hint(source, &arg))
            .collect(),
        span: span_for(source, hint.syntax().text_range()),
    }
}

fn attrs_from_cst(source: SourceId, attrs: AstChildren<SyntaxAttribute>) -> Vec<HirAttribute> {
    attrs
        .filter_map(|attr| attr_from_cst(source, &attr))
        .collect()
}

fn attr_from_cst(source: SourceId, attr: &SyntaxAttribute) -> Option<HirAttribute> {
    Some(HirAttribute {
        name: attr.path_text()?,
        value: attr_value(attr),
        span: span_for(source, attr.syntax().text_range()),
    })
}

fn attr_value(attr: &SyntaxAttribute) -> Option<String> {
    let values = attr
        .arguments()
        .filter_map(|arg| {
            let value = normalize_attr_value(arg.value_text()?);
            Some(match arg.name_text() {
                Some(name) => format!("{name}={value}"),
                None => value,
            })
        })
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.join(","))
}

fn normalize_attr_value(value: String) -> String {
    let value = compact_attr_value_whitespace(&value);
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_owned()
    } else {
        value
    }
}

fn compact_attr_value_whitespace(value: &str) -> String {
    let mut compact = String::new();
    let mut quote = None;
    let mut escaped = false;
    for ch in value.chars() {
        match quote {
            Some(active_quote) => {
                compact.push(ch);
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == active_quote {
                    quote = None;
                }
            }
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                compact.push(ch);
            }
            None if ch.is_whitespace() => {}
            None => compact.push(ch),
        }
    }
    compact
}

fn declaration_header(
    source: SourceId,
    range: TextRange,
    kind: DeclarationKind,
    name: Option<String>,
    is_public: bool,
) -> Option<SyntaxItemHeader> {
    Some(SyntaxItemHeader::Declaration {
        kind,
        name: name?,
        visibility: visibility(is_public),
        span: span_for(source, range),
    })
}

fn visibility(is_public: bool) -> Visibility {
    if is_public {
        Visibility::Public
    } else {
        Visibility::Private
    }
}

fn span_for(source: SourceId, range: TextRange) -> Span {
    Span::new(source, range.start().into(), range.end().into())
}
