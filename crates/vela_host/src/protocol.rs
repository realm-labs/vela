/// Read-only collection operations understood by the host boundary.
///
/// These operations are semantic protocol identities rather than Vela
/// standard-library method IDs. Host adapters therefore stay independent of
/// the language spelling used to invoke the protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostCollectionQuery {
    Len,
    IsEmpty,
}

impl HostCollectionQuery {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Len => "len",
            Self::IsEmpty => "is_empty",
        }
    }
}
