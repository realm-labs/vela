use std::fmt;

macro_rules! mir_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(u32);

        impl $name {
            pub(crate) const fn from_index(index: u32) -> Self {
                Self(index)
            }

            pub(crate) const fn index(self) -> u32 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}{}", $prefix, self.0)
            }
        }
    };
}

mir_id!(MirFunctionId, "f");
mir_id!(MirBlockId, "bb");
mir_id!(MirLocalId, "l");
mir_id!(MirTempId, "t");
mir_id!(MirStatementId, "s");
mir_id!(MirGuardId, "g");
mir_id!(MirSafepointId, "sp");
mir_id!(MirDebugLocalId, "dl");

pub(crate) trait ArenaId: Copy {
    fn from_arena_index(index: u32) -> Self;
    fn arena_index(self) -> u32;
}

macro_rules! arena_id {
    ($name:ident) => {
        impl ArenaId for $name {
            fn from_arena_index(index: u32) -> Self {
                Self::from_index(index)
            }

            fn arena_index(self) -> u32 {
                self.index()
            }
        }
    };
}

arena_id!(MirFunctionId);
arena_id!(MirBlockId);
arena_id!(MirLocalId);
arena_id!(MirTempId);
arena_id!(MirStatementId);
arena_id!(MirGuardId);
arena_id!(MirSafepointId);
arena_id!(MirDebugLocalId);
