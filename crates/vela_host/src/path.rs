use vela_common::{FieldId, HostObjectId, HostTypeId};

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
    pub fn with_segment_capacity(root: HostRef, capacity: usize) -> Self {
        Self {
            root,
            segments: Vec::with_capacity(capacity),
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
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.segments.push(PathSegment::Key(key.into()));
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
    Key(String),
    VariantField(FieldId),
}

#[cfg(test)]
mod tests {
    use vela_common::{FieldId, HostObjectId, HostTypeId};

    use super::*;

    #[test]
    fn path_with_segment_capacity_matches_regular_builder() {
        let root = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3);

        let regular = HostPath::new(root)
            .field(FieldId::new(2))
            .variant_field(FieldId::new(5));
        let reserved = HostPath::with_segment_capacity(root, 2)
            .field(FieldId::new(2))
            .variant_field(FieldId::new(5));

        assert_eq!(reserved, regular);
    }
}
