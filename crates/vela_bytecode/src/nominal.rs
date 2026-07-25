use vela_common::ShapeId;
use vela_def::{FieldId, TypeId, VariantId};
use vela_mir::MirTypeContract;

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalTypeKind {
    Record,
    Enum,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalFieldDescriptor {
    pub id: FieldId,
    pub name: String,
    pub contract: Option<MirTypeContract>,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalVariantDescriptor {
    pub id: VariantId,
    pub name: String,
    pub fields: Vec<NominalFieldDescriptor>,
}

#[cfg_attr(
    feature = "artifact-codec",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalTypeDescriptor {
    pub id: TypeId,
    pub canonical_name: String,
    pub runtime_name: String,
    pub kind: NominalTypeKind,
    pub shape: Option<ShapeId>,
    pub fields: Vec<NominalFieldDescriptor>,
    pub variants: Vec<NominalVariantDescriptor>,
}
