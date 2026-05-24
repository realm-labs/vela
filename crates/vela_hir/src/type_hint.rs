use vela_common::Span;
use vela_syntax::{
    ConstItem, EnumItem, EnumVariantFields, ImplItem, Param, StructField, TraitItem, TypeHint,
};

use crate::{HirAttribute, HirNodeId, attributes::attrs_from_syntax};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirTypeHint {
    pub path: Vec<String>,
    pub span: Span,
}

impl HirTypeHint {
    #[must_use]
    pub fn from_syntax(hint: &TypeHint) -> Self {
        Self {
            path: hint.path.clone(),
            span: hint.span,
        }
    }

    #[must_use]
    pub fn display(&self) -> String {
        self.path.join(".")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamHint {
    pub name: String,
    pub span: Span,
    pub type_hint: Option<HirTypeHint>,
    pub default_value_span: Option<Span>,
}

impl ParamHint {
    #[must_use]
    pub fn from_syntax(param: &Param) -> Self {
        Self {
            name: param.name.clone(),
            span: param.span,
            type_hint: param.type_hint.as_ref().map(HirTypeHint::from_syntax),
            default_value_span: param.default_value.as_ref().map(|value| value.span),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionSignature {
    pub params: Vec<ParamHint>,
    pub return_type: Option<HirTypeHint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstMetadata {
    pub type_hint: Option<HirTypeHint>,
    pub value_span: Span,
}

impl ConstMetadata {
    #[must_use]
    pub fn from_syntax(item: &ConstItem) -> Self {
        Self {
            type_hint: item.type_hint.as_ref().map(HirTypeHint::from_syntax),
            value_span: item.value.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructFieldHint {
    pub attrs: Vec<HirAttribute>,
    pub name: String,
    pub span: Span,
    pub type_hint: Option<HirTypeHint>,
}

impl StructFieldHint {
    #[must_use]
    pub fn from_syntax(field: &StructField) -> Self {
        Self {
            attrs: attrs_from_syntax(&field.attrs),
            name: field.name.clone(),
            span: field.span,
            type_hint: field.type_hint.as_ref().map(HirTypeHint::from_syntax),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructShape {
    pub fields: Vec<StructFieldHint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumShape {
    pub variants: Vec<EnumVariantHint>,
}

impl EnumShape {
    #[must_use]
    pub fn from_syntax(item: &EnumItem) -> Self {
        Self {
            variants: item
                .variants
                .iter()
                .map(EnumVariantHint::from_syntax)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumVariantHint {
    pub attrs: Vec<HirAttribute>,
    pub name: String,
    pub span: Span,
    pub fields: EnumVariantFieldsHint,
}

impl EnumVariantHint {
    #[must_use]
    pub fn from_syntax(variant: &vela_syntax::EnumVariant) -> Self {
        let fields = match &variant.fields {
            EnumVariantFields::Unit => EnumVariantFieldsHint::Unit,
            EnumVariantFields::Tuple(params) => {
                EnumVariantFieldsHint::Tuple(params.iter().map(ParamHint::from_syntax).collect())
            }
            EnumVariantFields::Record(fields) => EnumVariantFieldsHint::Record(
                fields.iter().map(StructFieldHint::from_syntax).collect(),
            ),
        };
        Self {
            attrs: attrs_from_syntax(&variant.attrs),
            name: variant.name.clone(),
            span: variant.span,
            fields,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnumVariantFieldsHint {
    Unit,
    Tuple(Vec<ParamHint>),
    Record(Vec<StructFieldHint>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitShape {
    pub methods: Vec<TraitMethodMetadata>,
}

impl TraitShape {
    #[must_use]
    pub fn from_syntax(
        item: &TraitItem,
        default_method_nodes: Vec<Option<(HirNodeId, Span)>>,
    ) -> Self {
        Self {
            methods: item
                .methods
                .iter()
                .zip(default_method_nodes)
                .map(|(method, default_body)| {
                    let (default_body_node, default_body_span) =
                        default_body.map_or((None, None), |(node, span)| (Some(node), Some(span)));
                    TraitMethodMetadata {
                        attrs: attrs_from_syntax(&method.attrs),
                        name: method.name.clone(),
                        span: method.span,
                        signature: FunctionSignature {
                            params: method.params.iter().map(ParamHint::from_syntax).collect(),
                            return_type: method.return_type.as_ref().map(HirTypeHint::from_syntax),
                        },
                        has_default: method.has_default,
                        default_body_node,
                        default_body_span,
                    }
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitMethodMetadata {
    pub attrs: Vec<HirAttribute>,
    pub name: String,
    pub span: Span,
    pub signature: FunctionSignature,
    pub has_default: bool,
    pub default_body_node: Option<HirNodeId>,
    pub default_body_span: Option<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplMetadata {
    pub trait_path: Vec<String>,
    pub target_path: Vec<String>,
    pub methods: Vec<ImplMethodMetadata>,
}

impl ImplMetadata {
    #[must_use]
    pub fn from_syntax(item: &ImplItem, method_nodes: Vec<(HirNodeId, Span)>) -> Self {
        Self {
            trait_path: item.trait_path.clone(),
            target_path: item.target_path.clone(),
            methods: item
                .methods
                .iter()
                .zip(method_nodes)
                .map(|(method, (node, span))| ImplMethodMetadata {
                    node,
                    name: method.function.name.clone(),
                    signature: FunctionSignature {
                        params: method
                            .function
                            .params
                            .iter()
                            .map(ParamHint::from_syntax)
                            .collect(),
                        return_type: method
                            .function
                            .return_type
                            .as_ref()
                            .map(HirTypeHint::from_syntax),
                    },
                    span,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplMethodMetadata {
    pub node: HirNodeId,
    pub name: String,
    pub signature: FunctionSignature,
    pub span: Span,
}
