macro_rules! hir_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(u32);

        impl $name {
            #[must_use]
            pub const fn new(value: u32) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn get(self) -> u32 {
                self.0
            }
        }
    };
}

hir_id!(ModuleId);
hir_id!(HirNodeId);
hir_id!(HirExprId);
hir_id!(HirDeclId);
