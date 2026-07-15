use vela_common::{CallableAsyncness, Span};

use crate::{
    attributes::HirAttribute,
    ids::{HirBodyId, HirNodeId},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirTypeHint {
    pub path: Vec<String>,
    pub args: Vec<HirTypeHint>,
    pub span: Span,
}

impl HirTypeHint {
    pub const UNIT_PATH: &'static str = "()";

    #[must_use]
    pub fn display(&self) -> String {
        if self.path.as_slice() == [Self::UNIT_PATH] {
            if self.args.is_empty() {
                return Self::UNIT_PATH.to_owned();
            }
            let args = self
                .args
                .iter()
                .map(Self::display)
                .collect::<Vec<_>>()
                .join(", ");
            return format!("({args})");
        }

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
    /// The bound HIR body for an enum tuple-field default. Function parameter
    /// defaults are owned by [`crate::body::HirParam`] instead.
    pub default_body: Option<HirBodyId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionSignature {
    pub asyncness: CallableAsyncness,
    pub params: Vec<ParamHint>,
    pub return_type: Option<HirTypeHint>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstMetadata {
    pub type_hint: Option<HirTypeHint>,
    pub value_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateMetadata {
    pub storage: StateStorage,
    pub type_hint: HirTypeHint,
    pub initializer_span: Option<Span>,
    pub initializer_body: Option<HirBodyId>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StateStorage {
    Vm,
    Extern,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructFieldHint {
    pub attrs: Vec<HirAttribute>,
    pub name: String,
    pub span: Span,
    pub type_hint: Option<HirTypeHint>,
    pub default_value_span: Option<Span>,
    /// The field-owned HIR body for this schema default, when present.
    pub default_body: Option<HirBodyId>,
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
    pub name_span: Span,
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
    pub attrs: Vec<HirAttribute>,
    pub node: HirNodeId,
    pub name: String,
    pub name_span: Span,
    pub signature: FunctionSignature,
    pub span: Span,
    pub body_span: Span,
    pub visibility: crate::module_graph::Visibility,
}
