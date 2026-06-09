use vela_common::{FieldId, HostTypeId};

use crate::path::HostRef;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostTargetPlan {
    pub root_type: HostTypeId,
    pub parts: HostPathParts,
}

impl HostTargetPlan {
    #[must_use]
    pub fn new(root_type: HostTypeId) -> Self {
        Self {
            root_type,
            parts: HostPathParts::new(),
        }
    }

    #[must_use]
    pub fn with_part_capacity(root_type: HostTypeId, capacity: usize) -> Self {
        Self {
            root_type,
            parts: HostPathParts::with_capacity(capacity),
        }
    }

    #[must_use]
    pub fn field(mut self, field: FieldId) -> Self {
        self.parts.push(HostPathPart::Field(field));
        self
    }

    #[must_use]
    pub fn variant_field(mut self, field: FieldId) -> Self {
        self.parts.push(HostPathPart::VariantField(field));
        self
    }

    #[must_use]
    pub fn const_index(mut self, index: u32) -> Self {
        self.parts.push(HostPathPart::ConstIndex(index));
        self
    }

    #[must_use]
    pub fn const_key(mut self, key: impl Into<String>) -> Self {
        self.parts.push(HostPathPart::ConstKey(key.into()));
        self
    }

    #[must_use]
    pub fn dyn_index(mut self, arg: u8) -> Self {
        self.parts.push(HostPathPart::DynIndex { arg });
        self
    }

    #[must_use]
    pub fn dyn_key(mut self, arg: u8) -> Self {
        self.parts.push(HostPathPart::DynKey { arg });
        self
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostPathPart {
    Field(FieldId),
    VariantField(FieldId),
    ConstIndex(u32),
    ConstKey(String),
    DynIndex { arg: u8 },
    DynKey { arg: u8 },
}

#[derive(Clone, Debug)]
pub struct HostPathParts {
    inner: HostPathPartsStorage,
}

impl HostPathParts {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: HostPathPartsStorage::Empty,
        }
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: HostPathPartsStorage::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, part: HostPathPart) {
        self.inner.push(part);
    }

    #[must_use]
    pub fn as_slice(&self) -> &[HostPathPart] {
        self.inner.as_slice()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }

    #[must_use]
    pub fn storage_kind(&self) -> HostPathPartsStorageKind {
        self.inner.kind()
    }

    #[must_use]
    pub fn dynamic_arg_count(&self) -> usize {
        self.as_slice()
            .iter()
            .filter_map(|part| match part {
                HostPathPart::DynIndex { arg } | HostPathPart::DynKey { arg } => {
                    Some(usize::from(*arg) + 1)
                }
                HostPathPart::Field(_)
                | HostPathPart::VariantField(_)
                | HostPathPart::ConstIndex(_)
                | HostPathPart::ConstKey(_) => None,
            })
            .max()
            .unwrap_or(0)
    }
}

impl Default for HostPathParts {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for HostPathParts {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for HostPathParts {}

impl std::hash::Hash for HostPathParts {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(self.as_slice(), state);
    }
}

impl Ord for HostPathParts {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl PartialOrd for HostPathParts {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug)]
enum HostPathPartsStorage {
    Empty,
    One([HostPathPart; 1]),
    Two([HostPathPart; 2]),
    Three([HostPathPart; 3]),
    Four([HostPathPart; 4]),
    Many(Vec<HostPathPart>),
}

impl HostPathPartsStorage {
    fn with_capacity(capacity: usize) -> Self {
        if capacity > 4 {
            Self::Many(Vec::with_capacity(capacity))
        } else {
            Self::Empty
        }
    }

    fn push(&mut self, part: HostPathPart) {
        let current = std::mem::replace(self, Self::Empty);
        *self = match current {
            Self::Empty => Self::One([part]),
            Self::One([first]) => Self::Two([first, part]),
            Self::Two([first, second]) => Self::Three([first, second, part]),
            Self::Three([first, second, third]) => Self::Four([first, second, third, part]),
            Self::Four([first, second, third, fourth]) => {
                Self::Many(vec![first, second, third, fourth, part])
            }
            Self::Many(mut parts) => {
                parts.push(part);
                Self::Many(parts)
            }
        };
    }

    fn as_slice(&self) -> &[HostPathPart] {
        match self {
            Self::Empty => &[],
            Self::One(parts) => parts,
            Self::Two(parts) => parts,
            Self::Three(parts) => parts,
            Self::Four(parts) => parts,
            Self::Many(parts) => parts,
        }
    }

    fn kind(&self) -> HostPathPartsStorageKind {
        match self {
            Self::Empty => HostPathPartsStorageKind::Empty,
            Self::One(_) => HostPathPartsStorageKind::One,
            Self::Two(_) => HostPathPartsStorageKind::Two,
            Self::Three(_) => HostPathPartsStorageKind::Three,
            Self::Four(_) => HostPathPartsStorageKind::Four,
            Self::Many(_) => HostPathPartsStorageKind::Many,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostPathPartsStorageKind {
    Empty,
    One,
    Two,
    Three,
    Four,
    Many,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostPathArg<'a> {
    Index(u32),
    Key(&'a str),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostPathArgOwned {
    Index(u32),
    Key(String),
}

impl<'a> From<&'a HostPathArgOwned> for HostPathArg<'a> {
    fn from(arg: &'a HostPathArgOwned) -> Self {
        match arg {
            HostPathArgOwned::Index(index) => Self::Index(*index),
            HostPathArgOwned::Key(key) => Self::Key(key),
        }
    }
}

impl HostPathArg<'_> {
    #[must_use]
    pub fn to_owned_arg(self) -> HostPathArgOwned {
        match self {
            Self::Index(index) => HostPathArgOwned::Index(index),
            Self::Key(key) => HostPathArgOwned::Key(key.to_owned()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HostTargetInstance<'a> {
    pub root: HostRef,
    pub plan: &'a HostTargetPlan,
    pub args: &'a [HostPathArg<'a>],
}

impl<'a> HostTargetInstance<'a> {
    #[must_use]
    pub fn new(root: HostRef, plan: &'a HostTargetPlan, args: &'a [HostPathArg<'a>]) -> Self {
        Self { root, plan, args }
    }

    #[must_use]
    pub fn arg(&self, index: u8) -> Option<HostPathArg<'a>> {
        self.args.get(usize::from(index)).copied()
    }

    #[must_use]
    pub fn arg_index(&self, index: u8) -> Option<u32> {
        match self.arg(index) {
            Some(HostPathArg::Index(value)) => Some(value),
            Some(HostPathArg::Key(_)) | None => None,
        }
    }

    #[must_use]
    pub fn arg_key(&self, index: u8) -> Option<&'a str> {
        match self.arg(index) {
            Some(HostPathArg::Key(value)) => Some(value),
            Some(HostPathArg::Index(_)) | None => None,
        }
    }

    pub fn try_to_diagnostic_path(&self) -> Result<HostDiagnosticPath, MissingHostPathArg> {
        let mut segments = Vec::with_capacity(self.plan.parts.len());
        for part in self.plan.parts.as_slice() {
            let segment = match part {
                HostPathPart::Field(field) => HostDiagnosticSegment::Field(*field),
                HostPathPart::VariantField(field) => HostDiagnosticSegment::VariantField(*field),
                HostPathPart::ConstIndex(index) => HostDiagnosticSegment::Index(*index),
                HostPathPart::ConstKey(key) => HostDiagnosticSegment::Key(key.clone()),
                HostPathPart::DynIndex { arg } | HostPathPart::DynKey { arg } => {
                    match self.arg(*arg) {
                        Some(HostPathArg::Index(index)) => HostDiagnosticSegment::Index(index),
                        Some(HostPathArg::Key(key)) => HostDiagnosticSegment::Key(key.to_owned()),
                        None => return Err(MissingHostPathArg { index: *arg }),
                    }
                }
            };
            segments.push(segment);
        }
        Ok(HostDiagnosticPath {
            root: self.root,
            segments,
        })
    }

    #[must_use]
    pub fn to_diagnostic_path(&self) -> HostDiagnosticPath {
        self.try_to_diagnostic_path()
            .expect("host target instance missing dynamic path argument")
    }
}

impl From<&crate::path::HostPath> for HostTargetPlan {
    fn from(path: &crate::path::HostPath) -> Self {
        let mut plan = Self::with_part_capacity(path.root.type_id, path.segments.len());
        for segment in &path.segments {
            plan.parts.push(match segment {
                crate::path::PathSegment::Field(field) => HostPathPart::Field(*field),
                crate::path::PathSegment::Index(index) => HostPathPart::ConstIndex(*index),
                crate::path::PathSegment::Key(key) => HostPathPart::ConstKey(key.clone()),
                crate::path::PathSegment::VariantField(field) => HostPathPart::VariantField(*field),
            });
        }
        plan
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissingHostPathArg {
    pub index: u8,
}

impl std::fmt::Display for MissingHostPathArg {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "missing host path dynamic argument {}",
            self.index
        )
    }
}

impl std::error::Error for MissingHostPathArg {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostDiagnosticPath {
    pub root: HostRef,
    pub segments: Vec<HostDiagnosticSegment>,
}

impl HostDiagnosticPath {
    #[must_use]
    pub fn to_host_path(&self) -> crate::path::HostPath {
        let mut path = crate::path::HostPath::with_segment_capacity(self.root, self.segments.len());
        for segment in &self.segments {
            path.segments.push(match segment {
                HostDiagnosticSegment::Field(field) => crate::path::PathSegment::Field(*field),
                HostDiagnosticSegment::Index(index) => crate::path::PathSegment::Index(*index),
                HostDiagnosticSegment::Key(key) => crate::path::PathSegment::Key(key.clone()),
                HostDiagnosticSegment::VariantField(field) => {
                    crate::path::PathSegment::VariantField(*field)
                }
            });
        }
        path
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostDiagnosticSegment {
    Field(FieldId),
    Index(u32),
    Key(String),
    VariantField(FieldId),
}

#[cfg(test)]
mod tests {
    use vela_common::{FieldId, HostObjectId, HostTypeId};

    use super::*;

    fn root() -> HostRef {
        HostRef::new(HostTypeId::new(1), HostObjectId::new(7), 3)
    }

    #[test]
    fn target_plan_identity_is_shape_based_not_object_based() {
        let plan = HostTargetPlan::new(HostTypeId::new(1)).field(FieldId::new(2));
        let same_shape = HostTargetPlan::new(HostTypeId::new(1)).field(FieldId::new(2));
        let other_root_type = HostTargetPlan::new(HostTypeId::new(9)).field(FieldId::new(2));

        assert_eq!(plan, same_shape);
        assert_ne!(plan, other_root_type);
    }

    #[test]
    fn target_parts_use_inline_storage_for_common_paths() {
        let plan = HostTargetPlan::new(HostTypeId::new(1))
            .field(FieldId::new(2))
            .const_index(4)
            .variant_field(FieldId::new(5));

        assert_eq!(plan.parts.storage_kind(), HostPathPartsStorageKind::Three);
        assert_eq!(
            plan.parts.as_slice(),
            &[
                HostPathPart::Field(FieldId::new(2)),
                HostPathPart::ConstIndex(4),
                HostPathPart::VariantField(FieldId::new(5)),
            ]
        );
    }

    #[test]
    fn target_parts_spill_long_paths_to_vec() {
        let plan = HostTargetPlan::new(HostTypeId::new(1))
            .field(FieldId::new(1))
            .field(FieldId::new(2))
            .field(FieldId::new(3))
            .field(FieldId::new(4))
            .field(FieldId::new(5));

        assert_eq!(plan.parts.storage_kind(), HostPathPartsStorageKind::Many);
        assert_eq!(plan.parts.len(), 5);
    }

    #[test]
    fn target_part_identity_ignores_storage_shape() {
        let inline = HostTargetPlan::new(HostTypeId::new(1)).field(FieldId::new(2));
        let reserved =
            HostTargetPlan::with_part_capacity(HostTypeId::new(1), 8).field(FieldId::new(2));

        assert_eq!(inline.parts.storage_kind(), HostPathPartsStorageKind::One);
        assert_eq!(
            reserved.parts.storage_kind(),
            HostPathPartsStorageKind::Many
        );
        assert_eq!(inline, reserved);
    }

    #[test]
    fn dynamic_arg_count_tracks_highest_placeholder() {
        let plan = HostTargetPlan::new(HostTypeId::new(1))
            .field(FieldId::new(1))
            .dyn_key(1)
            .dyn_index(0);

        assert_eq!(plan.parts.dynamic_arg_count(), 2);
    }

    #[test]
    fn diagnostic_path_materializes_static_and_dynamic_parts() {
        let plan = HostTargetPlan::new(HostTypeId::new(1))
            .field(FieldId::new(2))
            .dyn_index(0)
            .const_key("gold")
            .dyn_key(1);
        let args = [HostPathArg::Index(4), HostPathArg::Key("bonus")];
        let instance = HostTargetInstance::new(root(), &plan, &args);

        assert_eq!(
            instance.to_diagnostic_path(),
            HostDiagnosticPath {
                root: root(),
                segments: vec![
                    HostDiagnosticSegment::Field(FieldId::new(2)),
                    HostDiagnosticSegment::Index(4),
                    HostDiagnosticSegment::Key("gold".to_owned()),
                    HostDiagnosticSegment::Key("bonus".to_owned()),
                ],
            }
        );
    }

    #[test]
    fn diagnostic_path_reports_missing_dynamic_argument() {
        let plan = HostTargetPlan::new(HostTypeId::new(1)).dyn_key(0);
        let instance = HostTargetInstance::new(root(), &plan, &[]);

        assert_eq!(
            instance.try_to_diagnostic_path(),
            Err(MissingHostPathArg { index: 0 })
        );
    }

    #[test]
    fn owned_path_args_can_be_borrowed_for_instances() {
        let owned = [
            HostPathArgOwned::Index(9),
            HostPathArgOwned::Key("score".to_owned()),
        ];
        let borrowed = [HostPathArg::from(&owned[0]), HostPathArg::from(&owned[1])];

        assert_eq!(borrowed, [HostPathArg::Index(9), HostPathArg::Key("score")]);
        assert_eq!(borrowed[1].to_owned_arg(), owned[1]);
    }
}
