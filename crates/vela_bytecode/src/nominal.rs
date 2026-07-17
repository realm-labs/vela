use vela_common::ShapeId;
use vela_def::{FieldId, TypeId, VariantId};
use vela_mir::MirTypeContract;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalTypeKind {
    Record,
    Enum,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalFieldDescriptor {
    pub id: FieldId,
    pub name: String,
    pub contract: Option<MirTypeContract>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalVariantDescriptor {
    pub id: VariantId,
    pub name: String,
    pub fields: Vec<NominalFieldDescriptor>,
}

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
