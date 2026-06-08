use crate::InstructionOffset;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct CacheSiteId(u32);

impl CacheSiteId {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CacheSiteKind {
    GlobalRead,
    GlobalWrite,
    RecordFieldRead,
    RecordFieldWrite,
    MethodCall,
    HostPathRead,
    HostPathWrite,
    NativeCall,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheSiteDesc {
    pub id: CacheSiteId,
    pub kind: CacheSiteKind,
    pub function: String,
    pub instruction_offset: InstructionOffset,
}

impl CacheSiteDesc {
    #[must_use]
    pub fn new(
        id: CacheSiteId,
        kind: CacheSiteKind,
        function: impl Into<String>,
        instruction_offset: InstructionOffset,
    ) -> Self {
        Self {
            id,
            kind,
            function: function.into(),
            instruction_offset,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CacheSiteLayout {
    sites: Vec<CacheSiteDesc>,
}

impl CacheSiteLayout {
    #[must_use]
    pub fn new(sites: Vec<CacheSiteDesc>) -> Self {
        Self { sites }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sites.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.sites.len()
    }

    #[must_use]
    pub fn sites(&self) -> &[CacheSiteDesc] {
        &self.sites
    }

    #[must_use]
    pub fn get(&self, id: CacheSiteId) -> Option<&CacheSiteDesc> {
        self.sites.get(id.index())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_site_id_exposes_stable_index() {
        let id = CacheSiteId::new(7);

        assert_eq!(id.get(), 7);
        assert_eq!(id.index(), 7);
    }

    #[test]
    fn cache_site_layout_indexes_descriptors_by_id() {
        let layout = CacheSiteLayout::new(vec![
            CacheSiteDesc::new(
                CacheSiteId::new(0),
                CacheSiteKind::GlobalRead,
                "main",
                InstructionOffset(3),
            ),
            CacheSiteDesc::new(
                CacheSiteId::new(1),
                CacheSiteKind::RecordFieldRead,
                "main",
                InstructionOffset(9),
            ),
        ]);

        assert_eq!(layout.len(), 2);
        assert_eq!(
            layout.get(CacheSiteId::new(1)),
            Some(&CacheSiteDesc::new(
                CacheSiteId::new(1),
                CacheSiteKind::RecordFieldRead,
                "main",
                InstructionOffset(9),
            ))
        );
        assert_eq!(layout.get(CacheSiteId::new(2)), None);
    }
}
