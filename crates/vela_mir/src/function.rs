use std::collections::{BTreeMap, BTreeSet};

use vela_def::{FunctionId, MethodId, TypeId};
use vela_hir::binding::LocalBindingKind;
use vela_hir::ids::{HirBodyId, HirCaptureId, HirExprId, HirLocalId, HirParamId, HirScopeId};

use crate::arena::Arena;
use crate::{
    MirBasicBlock, MirBlockId, MirBuildError, MirDebugLocalId, MirFunctionId, MirGuard, MirGuardId,
    MirLiveValue, MirLocalId, MirSafepoint, MirSafepointId, MirSourceOrigin, MirStatement,
    MirStatementId, MirTempId, MirTerminator, MirTypeContract, MirValueType,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirFunctionOwner {
    Function(FunctionId),
    Method(crate::MethodExecutableTarget),
    Lambda {
        parent: MirFunctionId,
        expression: HirExprId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirLocalKind {
    Script(HirLocalId),
    Synthetic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirLocal {
    pub kind: MirLocalKind,
    pub value_type: MirValueType,
    pub origin: MirSourceOrigin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirParameterKind {
    Receiver,
    Explicit(HirParamId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirParameterSpec {
    pub hir_local: HirLocalId,
    pub kind: MirParameterKind,
    pub name: String,
    pub value_type: MirValueType,
    pub contract: Option<MirTypeContract>,
    pub default_body: Option<HirBodyId>,
    pub origin: MirSourceOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunctionParameter {
    pub storage: MirLocalId,
    pub hir_local: HirLocalId,
    pub kind: MirParameterKind,
    pub name: String,
    pub contract: Option<MirTypeContract>,
    pub default_body: Option<HirBodyId>,
    pub origin: MirSourceOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunctionCapture {
    pub storage: MirLocalId,
    pub capture: HirCaptureId,
    pub source_local: HirLocalId,
    pub name: String,
    pub origin: MirSourceOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunctionReturn {
    pub contract: MirTypeContract,
    pub origin: MirSourceOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirTemp {
    pub value_type: MirValueType,
    pub origin: MirSourceOrigin,
    definition: Option<MirStatementId>,
}

impl MirTemp {
    #[must_use]
    pub const fn definition(&self) -> Option<MirStatementId> {
        self.definition
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugLocalKind {
    Parameter,
    Local,
    LoopBinding,
    PatternBinding,
    Capture,
    Synthetic,
}

impl From<LocalBindingKind> for DebugLocalKind {
    fn from(kind: LocalBindingKind) -> Self {
        match kind {
            LocalBindingKind::Parameter | LocalBindingKind::LambdaParameter => Self::Parameter,
            LocalBindingKind::Let => Self::Local,
            LocalBindingKind::For => Self::LoopBinding,
            LocalBindingKind::Pattern => Self::PatternBinding,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirLiveRegion {
    pub blocks: BTreeSet<MirBlockId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirDebugLocal {
    pub storage: MirLocalId,
    pub name: String,
    pub kind: DebugLocalKind,
    pub hir_local: Option<HirLocalId>,
    pub scope: HirScopeId,
    pub origin: MirSourceOrigin,
    pub live_region: MirLiveRegion,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirLiveness {
    pub block_live_in: BTreeMap<MirBlockId, BTreeSet<MirLiveValue>>,
    pub block_live_out: BTreeMap<MirBlockId, BTreeSet<MirLiveValue>>,
    pub statement_live_before: BTreeMap<MirStatementId, BTreeSet<MirLiveValue>>,
    pub statement_live_after: BTreeMap<MirStatementId, BTreeSet<MirLiveValue>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MirFunction {
    body: HirBodyId,
    owner: MirFunctionOwner,
    code_symbol: String,
    origin: MirSourceOrigin,
    return_contract: Option<MirFunctionReturn>,
    parameters: Vec<MirFunctionParameter>,
    captures: Vec<MirFunctionCapture>,
    entry: MirBlockId,
    blocks: Arena<MirBlockId, MirBasicBlock>,
    locals: Arena<MirLocalId, MirLocal>,
    temps: Arena<MirTempId, MirTemp>,
    statements: Arena<MirStatementId, MirStatement>,
    guards: Arena<MirGuardId, MirGuard>,
    safepoints: Arena<MirSafepointId, MirSafepoint>,
    debug_locals: Arena<MirDebugLocalId, MirDebugLocal>,
    liveness: MirLiveness,
}

impl MirFunction {
    #[must_use]
    pub fn new(
        body: HirBodyId,
        owner: MirFunctionOwner,
        code_symbol: impl Into<String>,
        return_contract: Option<MirFunctionReturn>,
        origin: MirSourceOrigin,
    ) -> Self {
        let mut blocks = Arena::default();
        let entry = blocks.allocate(MirBasicBlock::default());
        Self {
            body,
            owner,
            code_symbol: code_symbol.into(),
            origin,
            return_contract,
            parameters: Vec::new(),
            captures: Vec::new(),
            entry,
            blocks,
            locals: Arena::default(),
            temps: Arena::default(),
            statements: Arena::default(),
            guards: Arena::default(),
            safepoints: Arena::default(),
            debug_locals: Arena::default(),
            liveness: MirLiveness::default(),
        }
    }

    #[must_use]
    pub const fn entry_block(&self) -> MirBlockId {
        self.entry
    }

    #[must_use]
    pub const fn body(&self) -> HirBodyId {
        self.body
    }

    #[must_use]
    pub const fn owner(&self) -> &MirFunctionOwner {
        &self.owner
    }

    #[must_use]
    pub fn code_symbol(&self) -> &str {
        &self.code_symbol
    }

    #[must_use]
    pub const fn origin(&self) -> MirSourceOrigin {
        self.origin
    }

    #[must_use]
    pub const fn return_contract(&self) -> Option<&MirFunctionReturn> {
        self.return_contract.as_ref()
    }

    pub fn add_block(&mut self) -> MirBlockId {
        self.blocks.allocate(MirBasicBlock::default())
    }

    pub fn add_script_local(
        &mut self,
        hir_local: HirLocalId,
        value_type: MirValueType,
        origin: MirSourceOrigin,
    ) -> MirLocalId {
        self.locals.allocate(MirLocal {
            kind: MirLocalKind::Script(hir_local),
            value_type,
            origin,
        })
    }

    pub fn add_parameter(&mut self, parameter: MirParameterSpec) -> MirLocalId {
        let storage =
            self.add_script_local(parameter.hir_local, parameter.value_type, parameter.origin);
        self.parameters.push(MirFunctionParameter {
            storage,
            hir_local: parameter.hir_local,
            kind: parameter.kind,
            name: parameter.name,
            contract: parameter.contract,
            default_body: parameter.default_body,
            origin: parameter.origin,
        });
        storage
    }

    pub fn add_capture(
        &mut self,
        capture: HirCaptureId,
        source_local: HirLocalId,
        name: impl Into<String>,
        value_type: MirValueType,
        origin: MirSourceOrigin,
    ) -> MirLocalId {
        let storage = self.add_script_local(source_local, value_type, origin);
        self.captures.push(MirFunctionCapture {
            storage,
            capture,
            source_local,
            name: name.into(),
            origin,
        });
        storage
    }

    pub fn add_synthetic_local(
        &mut self,
        value_type: MirValueType,
        origin: MirSourceOrigin,
    ) -> MirLocalId {
        self.locals.allocate(MirLocal {
            kind: MirLocalKind::Synthetic,
            value_type,
            origin,
        })
    }

    pub fn add_temp(&mut self, value_type: MirValueType, origin: MirSourceOrigin) -> MirTempId {
        self.temps.allocate(MirTemp {
            value_type,
            origin,
            definition: None,
        })
    }

    pub fn add_guard(&mut self, guard: MirGuard) -> MirGuardId {
        self.guards.allocate(guard)
    }

    pub fn add_safepoint(&mut self, safepoint: MirSafepoint) -> MirSafepointId {
        self.safepoints.allocate(safepoint)
    }

    pub fn add_debug_local(&mut self, local: MirDebugLocal) -> MirDebugLocalId {
        self.debug_locals.allocate(local)
    }

    pub fn append_statement(
        &mut self,
        block: MirBlockId,
        statement: MirStatement,
    ) -> Result<MirStatementId, MirBuildError> {
        let basic_block = self.blocks.get(block).ok_or(MirBuildError::MissingBlock {
            block,
            origin: statement.origin,
        })?;
        if basic_block.terminator().is_some() {
            return Err(MirBuildError::BlockAlreadyTerminated {
                block,
                origin: statement.origin,
            });
        }
        let required_effect = statement.kind.minimum_effect();
        if !statement.effect.contains(required_effect) {
            return Err(MirBuildError::IncompleteEffect {
                origin: statement.origin,
                required: required_effect,
                actual: statement.effect,
            });
        }
        if !statement.kind.has_valid_call_contract() {
            return Err(MirBuildError::InvalidCallArgumentPlacement {
                origin: statement.origin,
            });
        }
        if let crate::MirStatementKind::GuardTrap { guard, .. } = &statement.kind
            && self.guards.get(*guard).is_none()
        {
            return Err(MirBuildError::MissingGuard {
                guard: *guard,
                origin: statement.origin,
            });
        }
        match (
            statement.kind.destination_requirement(),
            statement.destination,
        ) {
            (crate::operations::MirDestinationRequirement::Required, None) => {
                return Err(MirBuildError::MissingStatementDestination {
                    origin: statement.origin,
                });
            }
            (crate::operations::MirDestinationRequirement::Forbidden, Some(_)) => {
                return Err(MirBuildError::UnexpectedStatementDestination {
                    origin: statement.origin,
                });
            }
            _ => {}
        }
        if (statement.effect.requires_safepoint() || statement.kind.requires_safepoint())
            && statement.safepoint.is_none()
        {
            return Err(MirBuildError::MissingSafepoint {
                origin: statement.origin,
            });
        }
        if let Some(safepoint) = statement.safepoint
            && self.safepoints.get(safepoint).is_none()
        {
            return Err(MirBuildError::MissingSafepoint {
                origin: statement.origin,
            });
        }
        if let Some(destination) = statement.destination {
            match destination {
                crate::MirPlace::Local(local) => {
                    if self.locals.get(local).is_none() {
                        return Err(MirBuildError::MissingLocal {
                            local,
                            origin: statement.origin,
                        });
                    }
                }
                crate::MirPlace::Temp(temp) => {
                    let temp_data = self.temps.get(temp).ok_or(MirBuildError::MissingTemp {
                        temp,
                        origin: statement.origin,
                    })?;
                    if temp_data.definition.is_some() {
                        return Err(MirBuildError::TempAlreadyDefined {
                            temp,
                            origin: statement.origin,
                        });
                    }
                }
            }
        }

        let destination = statement.destination;
        let statement_id = self.statements.allocate(statement);
        if let Some(crate::MirPlace::Temp(temp)) = destination
            && let Some(temp_data) = self.temps.get_mut(temp)
        {
            temp_data.definition = Some(statement_id);
        }
        if let Some(basic_block) = self.blocks.get_mut(block) {
            basic_block.push_statement(statement_id);
        }
        Ok(statement_id)
    }

    pub fn set_terminator(
        &mut self,
        block: MirBlockId,
        terminator: MirTerminator,
    ) -> Result<(), MirBuildError> {
        let required_effect = terminator.kind.minimum_effect();
        if !terminator.effect.contains(required_effect) {
            return Err(MirBuildError::IncompleteEffect {
                origin: terminator.origin,
                required: required_effect,
                actual: terminator.effect,
            });
        }
        if terminator.effect.requires_safepoint() && terminator.safepoint.is_none() {
            return Err(MirBuildError::MissingSafepoint {
                origin: terminator.origin,
            });
        }
        if let Some(safepoint) = terminator.safepoint
            && self.safepoints.get(safepoint).is_none()
        {
            return Err(MirBuildError::MissingSafepoint {
                origin: terminator.origin,
            });
        }
        let require_block = |target| {
            self.blocks
                .get(target)
                .ok_or(MirBuildError::MissingBlock {
                    block: target,
                    origin: terminator.origin,
                })
                .map(|_| ())
        };
        let require_local = |local| {
            self.locals
                .get(local)
                .ok_or(MirBuildError::MissingLocal {
                    local,
                    origin: terminator.origin,
                })
                .map(|_| ())
        };
        let require_guard = |guard| {
            self.guards
                .get(guard)
                .ok_or(MirBuildError::MissingGuard {
                    guard,
                    origin: terminator.origin,
                })
                .map(|_| ())
        };
        match &terminator.kind {
            crate::MirTerminatorKind::Jump(target) => require_block(*target)?,
            crate::MirTerminatorKind::Branch {
                then_block,
                else_block,
                ..
            } => {
                require_block(*then_block)?;
                require_block(*else_block)?;
            }
            crate::MirTerminatorKind::Switch {
                cases, otherwise, ..
            } => {
                for case in cases {
                    require_block(case.target)?;
                }
                require_block(*otherwise)?;
            }
            crate::MirTerminatorKind::GuardBranch {
                guard,
                passed,
                slow,
                ..
            } => {
                require_guard(*guard)?;
                require_block(*passed)?;
                require_block(*slow)?;
            }
            crate::MirTerminatorKind::IteratorNext {
                item, next, done, ..
            } => {
                require_local(*item)?;
                require_block(*next)?;
                require_block(*done)?;
            }
            crate::MirTerminatorKind::RangeNext {
                cursor,
                exhausted,
                item,
                next,
                done,
                ..
            } => {
                require_local(*cursor)?;
                require_local(*exhausted)?;
                require_local(*item)?;
                require_block(*next)?;
                require_block(*done)?;
            }
            crate::MirTerminatorKind::Return(_)
            | crate::MirTerminatorKind::Fail { .. }
            | crate::MirTerminatorKind::Unreachable => {}
        }
        let basic_block = self
            .blocks
            .get_mut(block)
            .ok_or(MirBuildError::MissingBlock {
                block,
                origin: terminator.origin,
            })?;
        if basic_block.terminator().is_some() {
            return Err(MirBuildError::BlockAlreadyTerminated {
                block,
                origin: terminator.origin,
            });
        }
        basic_block.set_terminator(terminator);
        Ok(())
    }

    #[must_use]
    pub fn block(&self, block: MirBlockId) -> Option<&MirBasicBlock> {
        self.blocks.get(block)
    }

    pub fn blocks(&self) -> impl Iterator<Item = (MirBlockId, &MirBasicBlock)> {
        self.blocks.iter()
    }

    #[must_use]
    pub fn local(&self, local: MirLocalId) -> Option<&MirLocal> {
        self.locals.get(local)
    }

    pub fn locals(&self) -> impl Iterator<Item = (MirLocalId, &MirLocal)> {
        self.locals.iter()
    }

    #[must_use]
    pub fn parameters(&self) -> &[MirFunctionParameter] {
        &self.parameters
    }

    #[must_use]
    pub fn captures(&self) -> &[MirFunctionCapture] {
        &self.captures
    }

    #[must_use]
    pub fn temp(&self, temp: MirTempId) -> Option<&MirTemp> {
        self.temps.get(temp)
    }

    pub fn temps(&self) -> impl Iterator<Item = (MirTempId, &MirTemp)> {
        self.temps.iter()
    }

    #[must_use]
    pub fn statement(&self, statement: MirStatementId) -> Option<&MirStatement> {
        self.statements.get(statement)
    }

    pub fn statements(&self) -> impl Iterator<Item = (MirStatementId, &MirStatement)> {
        self.statements.iter()
    }

    #[must_use]
    pub fn guard(&self, guard: MirGuardId) -> Option<&MirGuard> {
        self.guards.get(guard)
    }

    pub fn guards(&self) -> impl Iterator<Item = (MirGuardId, &MirGuard)> {
        self.guards.iter()
    }

    #[must_use]
    pub fn safepoint(&self, safepoint: MirSafepointId) -> Option<&MirSafepoint> {
        self.safepoints.get(safepoint)
    }

    pub fn safepoints(&self) -> impl Iterator<Item = (MirSafepointId, &MirSafepoint)> {
        self.safepoints.iter()
    }

    pub fn debug_locals(&self) -> impl Iterator<Item = (MirDebugLocalId, &MirDebugLocal)> {
        self.debug_locals.iter()
    }

    #[must_use]
    pub const fn liveness(&self) -> &MirLiveness {
        &self.liveness
    }

    pub fn set_liveness(&mut self, liveness: MirLiveness) {
        self.liveness = liveness;
    }

    /// Installs a statement without enforcing construction-time invariants.
    ///
    /// This exists only so verifier tests can prove malformed MIR is rejected;
    /// production construction must continue through [`Self::append_statement`].
    #[cfg(test)]
    pub(crate) fn verifier_test_append_statement_unchecked(
        &mut self,
        block: MirBlockId,
        statement: MirStatement,
    ) -> MirStatementId {
        let statement = self.statements.allocate(statement);
        self.blocks
            .get_mut(block)
            .expect("verifier corruption fixture block exists")
            .push_statement(statement);
        statement
    }

    /// Replaces a block terminator without construction-time validation.
    #[cfg(test)]
    pub(crate) fn verifier_test_set_terminator_unchecked(
        &mut self,
        block: MirBlockId,
        terminator: MirTerminator,
    ) {
        self.blocks
            .get_mut(block)
            .expect("verifier corruption fixture block exists")
            .set_terminator(terminator);
    }
}

/// Identity reserved for one generation-local MIR function slot.
///
/// Reservations let a parent body refer to a child lambda before either body
/// has been completely lowered. Stable runtime indexes are installed when the
/// slot is reserved, while [`MirProgram::function`] exposes only definitions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunctionReservation {
    body: HirBodyId,
    owner: MirFunctionOwner,
    origin: MirSourceOrigin,
}

impl MirFunctionReservation {
    #[must_use]
    pub const fn body(&self) -> HirBodyId {
        self.body
    }

    #[must_use]
    pub const fn owner(&self) -> &MirFunctionOwner {
        &self.owner
    }

    #[must_use]
    pub const fn origin(&self) -> MirSourceOrigin {
        self.origin
    }
}

#[derive(Clone, Debug, PartialEq)]
struct MirFunctionSlot {
    reservation: MirFunctionReservation,
    definition: Option<MirFunction>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MirProgram {
    targets: crate::MirTargetTable,
    functions: Arena<MirFunctionId, MirFunctionSlot>,
    functions_by_body: BTreeMap<HirBodyId, Vec<MirFunctionId>>,
    functions_by_id: BTreeMap<FunctionId, MirFunctionId>,
    methods_by_id: BTreeMap<(TypeId, MethodId), MirFunctionId>,
}

impl MirProgram {
    #[must_use]
    pub fn new(targets: crate::MirTargetTable) -> Self {
        Self {
            targets,
            ..Self::default()
        }
    }

    #[must_use]
    pub const fn targets(&self) -> &crate::MirTargetTable {
        &self.targets
    }

    /// Reserves a deterministic generation-local function ID and installs all
    /// stable body/function/method lookup indexes immediately.
    pub fn reserve_function(
        &mut self,
        body: HirBodyId,
        owner: MirFunctionOwner,
        origin: MirSourceOrigin,
    ) -> Result<MirFunctionId, MirBuildError> {
        match &owner {
            MirFunctionOwner::Function(function_id) => {
                if self.functions_by_id.contains_key(function_id) {
                    return Err(MirBuildError::DuplicateMirFunctionId {
                        function_id: *function_id,
                        origin,
                    });
                }
            }
            MirFunctionOwner::Method(target) => {
                if self.functions_by_id.contains_key(&target.function) {
                    return Err(MirBuildError::DuplicateMirFunctionId {
                        function_id: target.function,
                        origin,
                    });
                }
                if self
                    .methods_by_id
                    .contains_key(&(target.owner, target.method))
                {
                    return Err(MirBuildError::DuplicateMirMethodId {
                        owner: target.owner,
                        method_id: target.method,
                        origin,
                    });
                }
            }
            MirFunctionOwner::Lambda { parent, .. } => {
                if self.functions.get(*parent).is_none() {
                    return Err(MirBuildError::MissingMirFunction {
                        function: *parent,
                        origin,
                    });
                }
            }
        }
        let reservation = MirFunctionReservation {
            body,
            owner: owner.clone(),
            origin,
        };
        let id = self.functions.allocate(MirFunctionSlot {
            reservation,
            definition: None,
        });
        self.functions_by_body.entry(body).or_default().push(id);
        match owner {
            MirFunctionOwner::Function(function_id) => {
                self.functions_by_id.insert(function_id, id);
            }
            MirFunctionOwner::Method(target) => {
                self.functions_by_id.insert(target.function, id);
                self.methods_by_id.insert((target.owner, target.method), id);
            }
            MirFunctionOwner::Lambda { .. } => {}
        }
        Ok(id)
    }

    /// Defines a previously reserved function slot.
    pub fn define_function(
        &mut self,
        reservation: MirFunctionId,
        function: MirFunction,
    ) -> Result<(), MirBuildError> {
        let slot = self.functions.get_mut(reservation).ok_or(
            MirBuildError::MissingMirFunctionReservation {
                function: reservation,
                origin: function.origin,
            },
        )?;
        if slot.definition.is_some() {
            return Err(MirBuildError::MirFunctionAlreadyDefined {
                function: reservation,
                origin: function.origin,
            });
        }
        if slot.reservation.body != function.body {
            return Err(MirBuildError::MirFunctionReservationBodyMismatch {
                function: reservation,
                expected: slot.reservation.body,
                actual: function.body,
                origin: function.origin,
            });
        }
        if slot.reservation.owner != function.owner {
            return Err(MirBuildError::MirFunctionReservationOwnerMismatch {
                function: reservation,
                expected: Box::new(slot.reservation.owner.clone()),
                actual: Box::new(function.owner.clone()),
                origin: function.origin,
            });
        }
        slot.definition = Some(function);
        Ok(())
    }

    /// Reserves and immediately defines a complete function.
    pub fn add_function(&mut self, function: MirFunction) -> Result<MirFunctionId, MirBuildError> {
        let reservation =
            self.reserve_function(function.body, function.owner.clone(), function.origin)?;
        self.define_function(reservation, function)?;
        Ok(reservation)
    }

    #[must_use]
    pub fn function(&self, function: MirFunctionId) -> Option<&MirFunction> {
        self.functions.get(function)?.definition.as_ref()
    }

    #[must_use]
    pub fn reservation(&self, function: MirFunctionId) -> Option<&MirFunctionReservation> {
        self.functions.get(function).map(|slot| &slot.reservation)
    }

    #[must_use]
    pub fn function_by_id(&self, function: FunctionId) -> Option<MirFunctionId> {
        self.functions_by_id.get(&function).copied()
    }

    #[must_use]
    pub fn method_by_id(&self, owner: TypeId, method: MethodId) -> Option<MirFunctionId> {
        self.methods_by_id.get(&(owner, method)).copied()
    }

    pub fn functions_for_body(&self, body: HirBodyId) -> &[MirFunctionId] {
        self.functions_by_body.get(&body).map_or(&[], Vec::as_slice)
    }

    pub fn functions(&self) -> impl Iterator<Item = (MirFunctionId, &MirFunction)> {
        self.functions
            .iter()
            .filter_map(|(id, slot)| slot.definition.as_ref().map(|function| (id, function)))
    }

    pub fn reservations(&self) -> impl Iterator<Item = (MirFunctionId, &MirFunctionReservation)> {
        self.functions
            .iter()
            .map(|(id, slot)| (id, &slot.reservation))
    }

    pub fn undefined_reservations(
        &self,
    ) -> impl Iterator<Item = (MirFunctionId, &MirFunctionReservation)> {
        self.functions
            .iter()
            .filter_map(|(id, slot)| slot.definition.is_none().then_some((id, &slot.reservation)))
    }

    #[must_use]
    pub fn has_undefined_reservations(&self) -> bool {
        self.undefined_reservations().next().is_some()
    }

    #[must_use]
    pub fn defined_len(&self) -> usize {
        self.functions().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        // Reserved slots own generation-local IDs even before their bodies are
        // complete, so program length intentionally counts reservations.
        self.functions.len()
    }
}
