use vela_common::{FieldId, HostObjectId, HostTypeId, Symbol};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostRef {
    pub type_id: HostTypeId,
    pub object_id: HostObjectId,
    pub generation: u32,
}

impl HostRef {
    #[must_use]
    pub fn new(type_id: HostTypeId, object_id: HostObjectId, generation: u32) -> Self {
        Self {
            type_id,
            object_id,
            generation,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostPath {
    pub root: HostRef,
    pub segments: Vec<PathSegment>,
}

impl HostPath {
    #[must_use]
    pub fn new(root: HostRef) -> Self {
        Self {
            root,
            segments: Vec::new(),
        }
    }

    #[must_use]
    pub fn field(mut self, field: FieldId) -> Self {
        self.segments.push(PathSegment::Field(field));
        self
    }

    #[must_use]
    pub fn index(mut self, index: u32) -> Self {
        self.segments.push(PathSegment::Index(index));
        self
    }

    #[must_use]
    pub fn key(mut self, key: Symbol) -> Self {
        self.segments.push(PathSegment::Key(key));
        self
    }

    #[must_use]
    pub fn variant_field(mut self, field: FieldId) -> Self {
        self.segments.push(PathSegment::VariantField(field));
        self
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PathSegment {
    Field(FieldId),
    Index(u32),
    Key(Symbol),
    VariantField(FieldId),
}
