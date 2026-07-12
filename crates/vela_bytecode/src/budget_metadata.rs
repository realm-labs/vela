#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirBudgetCharge {
    pub site: vela_mir::MirBudgetSite,
    pub class: vela_mir::MirBudgetClass,
    pub units: u32,
}

impl crate::UnlinkedInstruction {
    #[must_use]
    pub const fn with_execution_units(mut self, units: u32) -> Self {
        self.execution_units = units;
        self
    }

    #[must_use]
    pub fn with_mir_metadata(
        mut self,
        origin: Option<vela_mir::MirBudgetSite>,
        charges: impl Into<Box<[MirBudgetCharge]>>,
    ) -> Self {
        self.mir_origin = origin;
        self.mir_budget_charges = charges.into();
        self
    }
}
