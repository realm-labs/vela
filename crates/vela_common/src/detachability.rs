//! Shared ownership fact for values crossing a detached execution boundary.

/// Why a statically known value cannot cross into an isolated Runtime.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NonDetachableValueKind {
    HostReference,
    BorrowedView,
    Iterator,
    Callable,
    RuntimeCapability,
}

impl NonDetachableValueKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HostReference => "host reference",
            Self::BorrowedView => "borrowed view",
            Self::Iterator => "iterator",
            Self::Callable => "callable",
            Self::RuntimeCapability => "runtime capability",
        }
    }
}

/// Static proof carried by compiler, artifact, and runtime task contracts.
///
/// `RuntimeChecked` is intentional: `Any`, reflection, and opaque registered
/// storage can hide either an owned value or a scoped capability. Admission
/// must inspect those values before publishing the child task.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Detachability {
    Detachable,
    RuntimeChecked,
    NonDetachable(NonDetachableValueKind),
}

impl Detachability {
    /// Combines recursively contained value facts. A known rejection wins;
    /// otherwise any erased edge keeps the complete value runtime-checked.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        match (self, other) {
            (Self::NonDetachable(kind), _) | (_, Self::NonDetachable(kind)) => {
                Self::NonDetachable(kind)
            }
            (Self::RuntimeChecked, _) | (_, Self::RuntimeChecked) => Self::RuntimeChecked,
            (Self::Detachable, Self::Detachable) => Self::Detachable,
        }
    }

    #[must_use]
    pub const fn requires_runtime_check(self) -> bool {
        matches!(self, Self::RuntimeChecked)
    }

    #[must_use]
    pub const fn rejection(self) -> Option<NonDetachableValueKind> {
        match self {
            Self::NonDetachable(kind) => Some(kind),
            Self::Detachable | Self::RuntimeChecked => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recursive_union_never_hides_a_rejection_or_runtime_check() {
        assert_eq!(
            Detachability::Detachable.union(Detachability::RuntimeChecked),
            Detachability::RuntimeChecked
        );
        assert_eq!(
            Detachability::RuntimeChecked.union(Detachability::NonDetachable(
                NonDetachableValueKind::HostReference,
            )),
            Detachability::NonDetachable(NonDetachableValueKind::HostReference)
        );
    }
}
