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
    pub fn key(mut self, key: Symbol) -> Self {
        self.segments.push(PathSegment::Key(key));
        self
    }

    #[must_use]
    pub fn variant_field(mut self, field: FieldId) -> Self {
        self.segments.push(PathSegment::VariantField(field));
        self
    }

    #[must_use]
    pub fn path_key(&self) -> HostPathKey {
        HostPathKey::from_path(self)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PathSegment {
    Field(FieldId),
    Index(u32),
    Key(Symbol),
    VariantField(FieldId),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostPathKey {
    pub root: HostRef,
    pub segments: PathKeySegments,
}

impl HostPathKey {
    #[must_use]
    pub fn new(root: HostRef) -> Self {
        Self {
            root,
            segments: PathKeySegments::Empty,
        }
    }

    #[must_use]
    pub fn with_segment_capacity(root: HostRef, capacity: usize) -> Self {
        Self {
            root,
            segments: PathKeySegments::with_capacity(capacity),
        }
    }

    #[must_use]
    pub fn from_path(path: &HostPath) -> Self {
        let mut key = Self::with_segment_capacity(path.root, path.segments.len());
        for segment in &path.segments {
            key = match segment {
                PathSegment::Field(field) => key.field(*field),
                PathSegment::Index(index) => key.index(*index),
                PathSegment::Key(symbol) => key.key(*symbol),
                PathSegment::VariantField(field) => key.variant_field(*field),
            };
        }
        key
    }

    #[must_use]
    pub fn field(mut self, field: FieldId) -> Self {
        self.segments.push(PathKeySegment::Field(field));
        self
    }

    #[must_use]
    pub fn index(mut self, index: u32) -> Self {
        self.segments.push(PathKeySegment::Index(index));
        self
    }

    #[must_use]
    pub fn key(mut self, key: Symbol) -> Self {
        self.segments.push(PathKeySegment::Key(key));
        self
    }

    #[must_use]
    pub fn variant_field(mut self, field: FieldId) -> Self {
        self.segments.push(PathKeySegment::VariantField(field));
        self
    }

    #[must_use]
    pub fn segments(&self) -> &[PathKeySegment] {
        self.segments.as_slice()
    }
}

impl From<&HostPath> for HostPathKey {
    fn from(path: &HostPath) -> Self {
        Self::from_path(path)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PathKeySegment {
    Field(FieldId),
    Index(u32),
    Key(Symbol),
    VariantField(FieldId),
}

#[derive(Clone, Debug)]
pub enum PathKeySegments {
    Empty,
    One([PathKeySegment; 1]),
    Two([PathKeySegment; 2]),
    Three([PathKeySegment; 3]),
    Four([PathKeySegment; 4]),
    Many(Vec<PathKeySegment>),
}

impl PathKeySegments {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        if capacity > 4 {
            Self::Many(Vec::with_capacity(capacity))
        } else {
            Self::Empty
        }
    }

    pub fn push(&mut self, segment: PathKeySegment) {
        let current = std::mem::replace(self, Self::Empty);
        *self = match current {
            Self::Empty => Self::One([segment]),
            Self::One([first]) => Self::Two([first, segment]),
            Self::Two([first, second]) => Self::Three([first, second, segment]),
            Self::Three([first, second, third]) => Self::Four([first, second, third, segment]),
            Self::Four([first, second, third, fourth]) => {
                Self::Many(vec![first, second, third, fourth, segment])
            }
            Self::Many(mut segments) => {
                segments.push(segment);
                Self::Many(segments)
            }
        };
    }

    #[must_use]
    pub fn as_slice(&self) -> &[PathKeySegment] {
        match self {
            Self::Empty => &[],
            Self::One(segments) => segments,
            Self::Two(segments) => segments,
            Self::Three(segments) => segments,
            Self::Four(segments) => segments,
            Self::Many(segments) => segments,
        }
    }
}

impl PartialEq for PathKeySegments {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for PathKeySegments {}

impl std::hash::Hash for PathKeySegments {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(self.as_slice(), state);
    }
}

impl Ord for PathKeySegments {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl PartialOrd for PathKeySegments {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use vela_common::{FieldId, HostObjectId, HostTypeId, Symbol};

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

    #[test]
    fn host_path_key_matches_path_identity() {
        let root = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3);
        let path = HostPath::new(root)
            .field(FieldId::new(2))
            .index(4)
            .key(Symbol::new(NonZeroU32::new(9).expect("non-zero symbol")))
            .variant_field(FieldId::new(5));

        let key = path.path_key();

        assert_eq!(
            key,
            HostPathKey::new(root)
                .field(FieldId::new(2))
                .index(4)
                .key(Symbol::new(NonZeroU32::new(9).expect("non-zero symbol")))
                .variant_field(FieldId::new(5))
        );
    }

    #[test]
    fn host_path_key_uses_inline_segments_for_short_paths() {
        let root = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3);
        let key = HostPathKey::new(root)
            .field(FieldId::new(2))
            .index(4)
            .variant_field(FieldId::new(5));

        assert!(matches!(key.segments, PathKeySegments::Three(_)));
        assert_eq!(
            key.segments(),
            &[
                PathKeySegment::Field(FieldId::new(2)),
                PathKeySegment::Index(4),
                PathKeySegment::VariantField(FieldId::new(5))
            ]
        );
    }

    #[test]
    fn host_path_key_spills_long_paths_to_vec() {
        let root = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3);
        let key = HostPathKey::new(root)
            .field(FieldId::new(1))
            .field(FieldId::new(2))
            .field(FieldId::new(3))
            .field(FieldId::new(4))
            .field(FieldId::new(5));

        assert!(matches!(key.segments, PathKeySegments::Many(_)));
        assert_eq!(key.segments().len(), 5);
    }

    #[test]
    fn host_path_key_identity_ignores_storage_shape() {
        let root = HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3);
        let inline = HostPathKey::new(root).field(FieldId::new(2));
        let reserved = HostPathKey::with_segment_capacity(root, 8).field(FieldId::new(2));

        assert!(matches!(inline.segments, PathKeySegments::One(_)));
        assert!(matches!(reserved.segments, PathKeySegments::Many(_)));
        assert_eq!(inline, reserved);
    }
}
