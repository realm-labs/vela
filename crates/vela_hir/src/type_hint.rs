use vela_common::Span;

use crate::{attributes::HirAttribute, ids::HirNodeId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirTypeHint {
    pub path: Vec<String>,
    pub args: Vec<HirTypeHint>,
    pub span: Span,
}

impl HirTypeHint {
    #[must_use]
    pub fn display(&self) -> String {
        let path = self.path.join("::");
        if self.args.is_empty() {
            path
        } else {
            let args = self
                .args
                .iter()
                .map(Self::display)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{path}<{args}>")
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamHint {
    pub name: String,
    pub span: Span,
    pub type_hint: Option<HirTypeHint>,
    pub default_value_span: Option<Span>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalMetadata {
    pub type_hint: HirTypeHint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructFieldHint {
    pub attrs: Vec<HirAttribute>,
    pub name: String,
    pub span: Span,
    pub type_hint: Option<HirTypeHint>,
    pub default_value_span: Option<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructShape {
    pub fields: Vec<StructFieldHint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumShape {
    pub variants: Vec<EnumVariantHint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumVariantHint {
    pub attrs: Vec<HirAttribute>,
    pub name: String,
    pub span: Span,
    pub fields: EnumVariantFieldsHint,
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
    pub kind: ImplMetadataKind,
    pub target_path: Vec<String>,
    pub methods: Vec<ImplMethodMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImplMetadataKind {
    Inherent,
    Trait { trait_path: Vec<String> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplMethodMetadata {
    pub node: HirNodeId,
    pub name: String,
    pub signature: FunctionSignature,
    pub span: Span,
    pub body_span: Span,
}
