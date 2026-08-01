use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use vela_common::HostMethodId;
use vela_def::{FunctionId, MethodId, TypeId, VariantId};
use vela_registry::DefinitionRegistry;

use crate::linked::{
    DynamicCallArgumentLinked, GuardContext, Instruction, InstructionKind, LinkedCodeObject,
    LinkedFrameDebugInfo, LinkedFrameSlotInfo, LinkedMethodDispatch, LinkedMethodDispatchKind,
    LinkedNativeFunction, LinkedProgram, LinkedType, LinkedVariant, TypeGuard, TypeGuardPlan,
};
use crate::{
    CacheSiteInstruction, Constant, FieldSlot, FunctionIndex, HostTargetPlanId, InstructionOffset,
    MethodDispatchHandle, NativeHandle, ScriptFunctionHandle, TypeHandle, UnlinkedCodeObject,
    UnlinkedInstruction, UnlinkedInstructionKind, UnlinkedProgram, VariantHandle,
};

mod support;
mod targets;

use support::{
    LinkContext, LinkInstructionContext, MethodDispatchKey, cache_site_at, sorted_field_slots,
};

#[derive(Clone, Debug, Default)]
pub struct Linker<'registry> {
    registry: Option<&'registry DefinitionRegistry>,
    native_implementations: BTreeSet<FunctionId>,
    internal_native_implementations: BTreeMap<FunctionId, vela_common::CallableAsyncness>,
}

impl<'registry> Linker<'registry> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_registry(registry: &'registry DefinitionRegistry) -> Self {
        Self {
            registry: Some(registry),
            native_implementations: BTreeSet::new(),
            internal_native_implementations: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_native_implementation(mut self, id: FunctionId) -> Self {
        self.native_implementations.insert(id);
        self
    }

    pub fn add_native_implementation(&mut self, id: FunctionId) {
        self.native_implementations.insert(id);
    }

    /// Registers an engine-internal native that is intentionally absent from
    /// the source-visible definition registry.
    #[doc(hidden)]
    pub fn add_internal_native_implementation(
        &mut self,
        id: FunctionId,
        asyncness: vela_common::CallableAsyncness,
    ) {
        self.internal_native_implementations.insert(id, asyncness);
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn link_test_program(
        &self,
        program: &UnlinkedProgram,
    ) -> Result<Arc<crate::LinkedArtifact>, LinkError> {
        Ok(Arc::new(
            self.link_unowned(program, None)?.0.into_test_artifact(),
        ))
    }

    fn link_unowned(
        &self,
        program: &UnlinkedProgram,
        package_metadata: Option<crate::PackageCompilationMetadata>,
    ) -> Result<
        (
            crate::artifact::UnboundLinkedProgram,
            Option<crate::PackageArtifactMetadata>,
        ),
        LinkError,
    > {
        let image = crate::ProgramImage::from_program(program);
        let mut linked = LinkContext::new(self, &image).link_program(&image)?;
        let package_metadata = package_metadata
            .map(|metadata| metadata.link(&mut linked))
            .transpose()
            .map_err(LinkError::PackageMetadata)?;
        let artifact = crate::LinkedArtifact::finish_unbound(image, linked)
            .map_err(LinkError::Verification)?;
        Ok((artifact, package_metadata))
    }

    /// Links one cohesive bytecode and verified-MIR compile generation.
    ///
    /// Handcrafted bytecode cannot enter this production artifact path:
    ///
    /// ```compile_fail
    /// use vela_bytecode::{Linker, UnlinkedProgram};
    /// let program = UnlinkedProgram::new();
    /// let _ = Linker::new().link_compiled_program(program);
    /// ```
    pub fn link_compiled_program(
        &self,
        program: crate::compiler::CompiledProgram,
    ) -> Result<Arc<crate::LinkedArtifact>, LinkError> {
        let parts = program.into_linker_parts();
        let (linked, package_metadata) =
            self.link_unowned(&parts.bytecode, parts.package_metadata)?;
        linked
            .bind_compiled_mir(
                parts.verified_mir,
                parts.binding_schema,
                &parts.mir_executables,
                &parts.budget_layouts,
                package_metadata,
            )
            .map(Arc::new)
    }

    /// Binds a decoded, source-independent bytecode artifact against this
    /// process's exact native/type registry.
    ///
    /// Format version 3 portable artifacts are interpreter-only and therefore
    /// carry no process-local MIR/JIT layouts.
    #[cfg(feature = "artifact-codec")]
    pub fn link_portable_program(
        &self,
        program: crate::PortableCompiledProgram,
    ) -> Result<Arc<crate::LinkedArtifact>, LinkError> {
        let (linked, package_metadata) = self.link_unowned(&program.bytecode, None)?;
        debug_assert!(package_metadata.is_none());
        linked
            .bind_portable(
                program.binding_schema,
                program.required_features,
                program.task_targets,
            )
            .map(Arc::new)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinkError {
    Verification(crate::verification::VerificationError),
    PackageMetadata(String),
    UnresolvedNative {
        name: String,
        id: FunctionId,
    },
    MissingNativeImplementation {
        name: String,
        id: FunctionId,
    },
    MissingScriptFunction {
        name: String,
        id: FunctionId,
    },
    MissingTaskTarget {
        name: String,
        id: FunctionId,
    },
    InvalidTaskMetadata(String),
    MirExecutableCountMismatch {
        expected: usize,
        actual: usize,
    },
    MirExecutableIdentityMismatch {
        index: FunctionIndex,
        expected_root: FunctionId,
        expected_function: vela_mir::MirFunctionId,
        actual_root: Option<FunctionId>,
        actual_function: Option<vela_mir::MirFunctionId>,
    },
    MissingMirRoot {
        root: FunctionId,
    },
    MissingMirFunction {
        root: FunctionId,
        function: vela_mir::MirFunctionId,
    },
    MissingMirBudgetCharge {
        executable: usize,
        site: vela_mir::MirBudgetSite,
    },
    MissingMirBudgetLayoutSite {
        executable: usize,
        site: vela_mir::MirBudgetSite,
    },
    ExtraMirBudgetLayoutSite {
        executable: usize,
    },
    DuplicateMirBudgetLayoutSite {
        executable: usize,
    },
    MirBudgetLayoutMismatch {
        executable: usize,
        site: vela_mir::MirBudgetSite,
        offset: InstructionOffset,
    },
    ExtraMirBudgetCharge {
        executable: usize,
        offset: InstructionOffset,
        site: vela_mir::MirBudgetSite,
    },
    DuplicateMirBudgetCharge {
        executable: usize,
        site: vela_mir::MirBudgetSite,
    },
    MirBudgetChargeMismatch {
        executable: usize,
        site: vela_mir::MirBudgetSite,
        expected: vela_mir::MirBudgetPoint,
        actual_class: vela_mir::MirBudgetClass,
        actual_units: u32,
    },
    MirBudgetPlacementMismatch {
        executable: usize,
        offset: InstructionOffset,
        site: vela_mir::MirBudgetSite,
        origin: Option<vela_mir::MirBudgetSite>,
    },
    MirBudgetEncodingMismatch {
        executable: usize,
        offset: InstructionOffset,
        encoded_units: u64,
        mapped_units: u64,
    },
    MirBudgetTotalMismatch {
        executable: usize,
        expected_units: u64,
        actual_units: u64,
    },
    InvalidNestedFunction {
        function: String,
        index: FunctionIndex,
    },
    MissingMethodDefinition {
        method: String,
        id: MethodId,
    },
    MissingState {
        function: String,
        state: String,
    },
    InvalidHostTarget {
        function: String,
        target: HostTargetPlanId,
    },
    UnresolvedType {
        name: String,
    },
    UnresolvedVariant {
        enum_name: String,
        variant: String,
    },
}

impl fmt::Display for LinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verification(error) => write!(formatter, "{error}"),
            Self::PackageMetadata(message) => formatter.write_str(message),
            Self::UnresolvedNative { name, id } => {
                write!(formatter, "unresolved native function {name} ({id:?})")
            }
            Self::MissingNativeImplementation { name, id } => {
                write!(
                    formatter,
                    "missing native implementation for {name} ({id:?})"
                )
            }
            Self::MissingScriptFunction { name, id } => {
                write!(formatter, "missing script function {name} ({id:?})")
            }
            Self::MissingTaskTarget { name, id } => {
                write!(formatter, "missing linked task target {name} ({id:?})")
            }
            Self::InvalidTaskMetadata(message) => {
                write!(formatter, "invalid linked task metadata: {message}")
            }
            Self::MirExecutableCountMismatch { expected, actual } => {
                write!(
                    formatter,
                    "linked artifact has {actual} executables, expected {expected} from its compiled MIR generation"
                )
            }
            Self::MirExecutableIdentityMismatch {
                index,
                expected_root,
                expected_function,
                actual_root,
                actual_function,
            } => write!(
                formatter,
                "compiled executable {index:?} has MIR identity {actual_root:?}/{actual_function:?}, expected root {expected_root:?} function {expected_function:?}"
            ),
            Self::MissingMirRoot { root } => {
                write!(formatter, "missing verified MIR root {root:?}")
            }
            Self::MissingMirFunction { root, function } => write!(
                formatter,
                "missing verified MIR function {function:?} in root {root:?}"
            ),
            Self::MissingMirBudgetCharge { executable, site } => write!(
                formatter,
                "linked executable {executable} is missing MIR budget charge {site:?}"
            ),
            Self::MissingMirBudgetLayoutSite { executable, site } => write!(
                formatter,
                "linked executable {executable} has no sealed layout for MIR budget site {site:?}"
            ),
            Self::ExtraMirBudgetLayoutSite { executable } => write!(
                formatter,
                "linked executable {executable} has an extra sealed MIR budget layout site"
            ),
            Self::DuplicateMirBudgetLayoutSite { executable } => write!(
                formatter,
                "linked executable {executable} has a duplicate sealed MIR budget layout site"
            ),
            Self::MirBudgetLayoutMismatch {
                executable,
                site,
                offset,
            } => write!(
                formatter,
                "linked executable {executable} does not implement sealed MIR budget site {site:?} at {offset:?}"
            ),
            Self::ExtraMirBudgetCharge {
                executable,
                offset,
                site,
            } => write!(
                formatter,
                "linked executable {executable} has extra MIR budget charge {site:?} at {offset:?}"
            ),
            Self::DuplicateMirBudgetCharge { executable, site } => write!(
                formatter,
                "linked executable {executable} duplicates MIR budget charge {site:?}"
            ),
            Self::MirBudgetChargeMismatch {
                executable,
                site,
                expected,
                actual_class,
                actual_units,
            } => write!(
                formatter,
                "linked executable {executable} maps MIR budget charge {site:?} as {actual_class:?}/{actual_units}, expected {:?}/{}",
                expected.class, expected.units
            ),
            Self::MirBudgetPlacementMismatch {
                executable,
                offset,
                site,
                origin,
            } => write!(
                formatter,
                "linked executable {executable} places MIR budget charge {site:?} at {offset:?} with origin {origin:?}"
            ),
            Self::MirBudgetEncodingMismatch {
                executable,
                offset,
                encoded_units,
                mapped_units,
            } => write!(
                formatter,
                "linked executable {executable} encodes {encoded_units} units at {offset:?} but maps {mapped_units}"
            ),
            Self::MirBudgetTotalMismatch {
                executable,
                expected_units,
                actual_units,
            } => write!(
                formatter,
                "linked executable {executable} has {actual_units} execution units, expected {expected_units} from verified MIR"
            ),
            Self::InvalidNestedFunction { function, index } => {
                write!(
                    formatter,
                    "function {function} references missing nested function {index:?}"
                )
            }
            Self::MissingMethodDefinition { method, id } => {
                write!(formatter, "missing method definition for {method} ({id:?})")
            }
            Self::MissingState { function, state } => {
                write!(
                    formatter,
                    "function {function} references missing state {state}"
                )
            }
            Self::InvalidHostTarget { function, target } => {
                write!(
                    formatter,
                    "function {function} references missing host target {target:?}"
                )
            }
            Self::UnresolvedType { name } => {
                write!(formatter, "unresolved type {name}")
            }
            Self::UnresolvedVariant { enum_name, variant } => {
                write!(formatter, "unresolved variant {enum_name}::{variant}")
            }
        }
    }
}

impl Error for LinkError {}

impl<'linker, 'registry> LinkContext<'linker, 'registry> {
    fn new(linker: &'linker Linker<'registry>, program: &crate::ProgramImage) -> Self {
        let mut script_functions_by_name = BTreeMap::new();
        let mut script_function_name_counts = BTreeMap::<String, usize>::new();
        let mut script_functions_by_id = BTreeMap::new();
        for (index, name) in program.entry_function_names().enumerate() {
            let handle = ScriptFunctionHandle::new(index);
            script_functions_by_name.insert(name.to_owned(), handle);
            *script_function_name_counts
                .entry(name.to_owned())
                .or_default() += 1;
        }
        script_functions_by_name.retain(|name, _| script_function_name_counts[name] == 1);
        for (id, index) in program.entry_function_ids() {
            script_functions_by_id.insert(id, ScriptFunctionHandle::new(index.0));
        }

        let mut script_methods_by_id = BTreeMap::new();
        for (_, _, _, method) in program.script_methods().methods() {
            if let Some(function) = script_functions_by_id
                .get(&method.function_id)
                .or_else(|| script_functions_by_name.get(&method.function))
            {
                script_methods_by_id.insert(method.id, *function);
            }
        }
        Self {
            linker,
            linked: LinkedProgram::new(),
            script_functions_by_name,
            script_functions_by_id,
            script_methods_by_id,
            native_handles: BTreeMap::new(),
            method_handles: BTreeMap::new(),
            type_handles: BTreeMap::new(),
            variant_handles: BTreeMap::new(),
        }
    }

    fn link_program(mut self, program: &crate::ProgramImage) -> Result<LinkedProgram, LinkError> {
        if let Some(metadata) = program.script_metadata() {
            self.linked.set_script_metadata(metadata.clone());
        }
        for descriptor in program.nominal_types() {
            self.linked.insert_nominal_type(descriptor.clone());
        }
        let mut functions = Vec::with_capacity(program.function_count());
        for (_, code) in program.functions() {
            functions.push(self.link_code(program, code)?);
        }

        for code in functions {
            self.linked.push_function(code);
        }
        for state in program.states() {
            let initializer = state
                .initializer
                .map(|function| {
                    self.script_functions_by_id
                        .get(&function)
                        .copied()
                        .ok_or_else(|| LinkError::MissingScriptFunction {
                            name: state.qualified_name.clone(),
                            id: function,
                        })
                })
                .transpose()?;
            self.linked.push_state(crate::LinkedStateDescriptor {
                id: state.id,
                qualified_name: state.qualified_name.clone(),
                visibility: state.visibility,
                storage: state.storage,
                type_contract: state.type_contract.clone(),
                initializer,
                source_span: state.source_span,
            });
        }
        self.link_script_method_dispatches(program)?;

        for name in program.entry_function_names() {
            let debug_name = self.linked.intern_debug_name(name.to_owned());
            if let Some(function) = self.script_functions_by_name.get(name).copied() {
                self.linked.set_entry_point(debug_name, function);
            }
        }
        for (id, function) in &self.script_functions_by_id {
            self.linked.set_entry_point_id(*id, *function);
        }

        Ok(self.linked)
    }

    fn link_script_method_dispatches(
        &mut self,
        program: &crate::ProgramImage,
    ) -> Result<(), LinkError> {
        for (owner, _type_name, method_name, method) in program.script_methods().methods() {
            let Some(function) = self
                .script_functions_by_id
                .get(&method.function_id)
                .or_else(|| self.script_functions_by_name.get(&method.function))
                .copied()
            else {
                continue;
            };
            let dispatch = self.intern_method_dispatch(
                MethodDispatchKey::Script(method.id, function),
                method_name.to_owned(),
            )?;
            self.linked
                .insert_script_method_dispatch(owner, method_name, dispatch);
        }
        Ok(())
    }

    fn link_code(
        &mut self,
        program: &crate::ProgramImage,
        code: &UnlinkedCodeObject,
    ) -> Result<LinkedCodeObject, LinkError> {
        let debug_name = self.linked.intern_debug_name(code.name.clone());
        let params = code
            .params
            .iter()
            .map(|param| self.linked.intern_debug_name(param.clone()))
            .collect::<Vec<_>>();
        let frame = self.link_frame(&code.frame);

        let mut linked = LinkedCodeObject::new(debug_name, code.register_count)
            .with_asyncness(code.asyncness)
            .with_params(params)
            .with_param_defaults(code.param_defaults.clone())
            .with_capture_count(code.capture_count);
        linked.frame = frame;
        linked.cache_sites = code.cache_sites.clone();
        linked.constants = code.constants.clone();
        for guard in &code.param_guards {
            let linked_guard = self.link_type_guard(guard.guard.clone(), &mut linked)?;
            linked.push_param_guard(guard.parameter, linked_guard);
        }
        if let Some(guard) = code.return_guard.clone() {
            let linked_guard = self.link_type_guard(guard, &mut linked)?;
            linked.set_return_guard(linked_guard);
        }
        let host_target_map = code
            .host_targets
            .iter()
            .cloned()
            .map(|target| linked.intern_host_target(target))
            .collect::<Vec<_>>();

        for (offset, instruction) in code.instructions.iter().enumerate() {
            let instruction = self.link_instruction(
                LinkInstructionContext {
                    program,
                    code,
                    host_target_map: &host_target_map,
                    linked_code: &mut linked,
                    instruction_offset: InstructionOffset(offset),
                },
                instruction,
            )?;
            linked.push_instruction(instruction);
        }

        Ok(linked)
    }

    fn link_frame(&mut self, frame: &crate::FrameDebugInfo) -> LinkedFrameDebugInfo {
        let mut linked = LinkedFrameDebugInfo::default();
        for slot in &frame.slots {
            linked.push_slot(LinkedFrameSlotInfo::new(
                self.linked.intern_debug_name(slot.name.clone()),
                slot.register,
                slot.span,
            ));
        }
        linked
    }

    fn link_instruction(
        &mut self,
        context: LinkInstructionContext<'_>,
        instruction: &UnlinkedInstruction,
    ) -> Result<Instruction, LinkError> {
        let program = context.program;
        let code = context.code;
        let host_target_map = context.host_target_map;
        let linked_code = context.linked_code;
        let instruction_offset = context.instruction_offset;
        let mut kind = match &instruction.kind {
            UnlinkedInstructionKind::ChargeExecutionUnits { units } => {
                InstructionKind::ChargeExecutionUnits { units: *units }
            }
            UnlinkedInstructionKind::LoadConst { dst, constant } => InstructionKind::LoadConst {
                dst: *dst,
                constant: *constant,
            },
            UnlinkedInstructionKind::Move { dst, src } => InstructionKind::Move {
                dst: *dst,
                src: *src,
            },
            UnlinkedInstructionKind::Not { dst, src } => InstructionKind::Not {
                dst: *dst,
                src: *src,
            },
            UnlinkedInstructionKind::Truthy { dst, src } => InstructionKind::Truthy {
                dst: *dst,
                src: *src,
            },
            UnlinkedInstructionKind::Negate { dst, src } => InstructionKind::Negate {
                dst: *dst,
                src: *src,
            },
            UnlinkedInstructionKind::Add { dst, lhs, rhs } => InstructionKind::Add {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::Sub { dst, lhs, rhs } => InstructionKind::Sub {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::Mul { dst, lhs, rhs } => InstructionKind::Mul {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::Div { dst, lhs, rhs } => InstructionKind::Div {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::Rem { dst, lhs, rhs } => InstructionKind::Rem {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::Equal { dst, lhs, rhs } => InstructionKind::Equal {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::NotEqual { dst, lhs, rhs } => InstructionKind::NotEqual {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::IdentityEqual { dst, lhs, rhs } => {
                InstructionKind::IdentityEqual {
                    dst: *dst,
                    lhs: *lhs,
                    rhs: *rhs,
                }
            }
            UnlinkedInstructionKind::IdentityNotEqual { dst, lhs, rhs } => {
                InstructionKind::IdentityNotEqual {
                    dst: *dst,
                    lhs: *lhs,
                    rhs: *rhs,
                }
            }
            UnlinkedInstructionKind::Less { dst, lhs, rhs } => InstructionKind::Less {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::LessEqual { dst, lhs, rhs } => InstructionKind::LessEqual {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::Greater { dst, lhs, rhs } => InstructionKind::Greater {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::GreaterEqual { dst, lhs, rhs } => {
                InstructionKind::GreaterEqual {
                    dst: *dst,
                    lhs: *lhs,
                    rhs: *rhs,
                }
            }
            UnlinkedInstructionKind::I64Add { dst, lhs, rhs } => InstructionKind::I64Add {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::I64Sub { dst, lhs, rhs } => InstructionKind::I64Sub {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::I64Mul { dst, lhs, rhs } => InstructionKind::I64Mul {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::I64Rem { dst, lhs, rhs } => InstructionKind::I64Rem {
                dst: *dst,
                lhs: *lhs,
                rhs: *rhs,
            },
            UnlinkedInstructionKind::I64AddImm { dst, lhs, imm } => InstructionKind::I64AddImm {
                dst: *dst,
                lhs: *lhs,
                imm: *imm,
            },
            UnlinkedInstructionKind::I64SubImm { dst, lhs, imm } => InstructionKind::I64SubImm {
                dst: *dst,
                lhs: *lhs,
                imm: *imm,
            },
            UnlinkedInstructionKind::I64MulImm { dst, lhs, imm } => InstructionKind::I64MulImm {
                dst: *dst,
                lhs: *lhs,
                imm: *imm,
            },
            UnlinkedInstructionKind::I64RemImm { dst, lhs, imm } => InstructionKind::I64RemImm {
                dst: *dst,
                lhs: *lhs,
                imm: *imm,
            },
            UnlinkedInstructionKind::I64CmpImm { dst, op, lhs, imm } => {
                InstructionKind::I64CmpImm {
                    dst: *dst,
                    op: *op,
                    lhs: *lhs,
                    imm: *imm,
                }
            }
            UnlinkedInstructionKind::I64CmpImmJumpIfFalse {
                op,
                lhs,
                imm,
                target,
            } => InstructionKind::I64CmpImmJumpIfFalse {
                op: *op,
                lhs: *lhs,
                imm: *imm,
                target: *target,
            },
            UnlinkedInstructionKind::BinaryIntLiteral {
                dst,
                op,
                value,
                literal,
                side,
            } => InstructionKind::BinaryIntLiteral {
                dst: *dst,
                op: *op,
                value: *value,
                magnitude: link_int_literal(literal),
                side: *side,
            },
            UnlinkedInstructionKind::BinaryFloatLiteral {
                dst,
                op,
                value,
                literal,
                side,
            } => InstructionKind::BinaryFloatLiteral {
                dst: *dst,
                op: *op,
                value: *value,
                literal: link_float_literal(literal),
                side: *side,
            },
            UnlinkedInstructionKind::JumpIfFalse { condition, target } => {
                InstructionKind::JumpIfFalse {
                    condition: *condition,
                    target: *target,
                }
            }
            UnlinkedInstructionKind::JumpIfNotMissing { value, target } => {
                InstructionKind::JumpIfNotMissing {
                    value: *value,
                    target: *target,
                }
            }
            UnlinkedInstructionKind::Jump { target } => InstructionKind::Jump { target: *target },
            UnlinkedInstructionKind::AwaitCall { operation, resume } => {
                let nested = UnlinkedInstruction::new((**operation).clone());
                let linked = self.link_instruction(
                    LinkInstructionContext {
                        program,
                        code,
                        host_target_map,
                        linked_code: &mut *linked_code,
                        instruction_offset,
                    },
                    &nested,
                )?;
                InstructionKind::AwaitCall {
                    operation: Box::new(linked.kind),
                    resume: *resume,
                }
            }
            UnlinkedInstructionKind::CallNative {
                dst,
                name,
                native,
                cache_site,
                args,
            } => {
                let native = self.link_native(name, *native)?;
                let debug_name = self.linked.intern_debug_name(name.clone());
                InstructionKind::CallNative {
                    dst: *dst,
                    native,
                    debug_name,
                    cache_site: *cache_site,
                    args: args.clone(),
                }
            }
            UnlinkedInstructionKind::CallFunction {
                dst,
                target,
                name,
                mode,
                args,
            } => {
                let function = self.resolve_script_function(*target, name)?;
                let debug_name = self.linked.intern_debug_name(name.clone());
                InstructionKind::CallFunction {
                    dst: *dst,
                    function,
                    debug_name,
                    mode: *mode,
                    args: args.clone(),
                }
            }
            UnlinkedInstructionKind::MakeClosure {
                dst,
                function,
                captures,
            } => {
                if program.function(*function).is_none() {
                    return Err(LinkError::InvalidNestedFunction {
                        function: code.name.clone(),
                        index: *function,
                    });
                }
                let function = ScriptFunctionHandle::new(function.0);
                InstructionKind::MakeClosure {
                    dst: *dst,
                    function,
                    captures: captures.clone(),
                }
            }
            UnlinkedInstructionKind::CallClosure { dst, callee, args } => {
                InstructionKind::CallClosure {
                    dst: *dst,
                    callee: *callee,
                    args: args.clone(),
                }
            }
            UnlinkedInstructionKind::CallDynamicMethod {
                dst,
                receiver,
                method,
                args,
            } => {
                let method_name = self.linked.intern_debug_name(method.clone());
                let args = args
                    .iter()
                    .map(|arg| DynamicCallArgumentLinked {
                        name: arg
                            .name
                            .as_ref()
                            .map(|name| self.linked.intern_debug_name(name.clone())),
                        value: arg.value,
                    })
                    .collect();
                InstructionKind::CallDynamicMethod {
                    dst: *dst,
                    receiver: *receiver,
                    method_name,
                    cache_site: cache_site_at(code, instruction_offset),
                    args,
                }
            }
            UnlinkedInstructionKind::CallMethodId {
                dst,
                receiver,
                method,
                method_id,
                args,
            } => {
                let dispatch = self.link_method_dispatch(method, *method_id)?;
                let debug_name = self.linked.intern_debug_name(method.clone());
                InstructionKind::CallMethod {
                    dst: *dst,
                    receiver: *receiver,
                    dispatch,
                    debug_name,
                    cache_site: cache_site_at(code, instruction_offset),
                    args: args.clone(),
                }
            }
            UnlinkedInstructionKind::TryPropagate { dst, src, expected } => {
                InstructionKind::TryPropagate {
                    dst: *dst,
                    src: *src,
                    expected: *expected,
                }
            }
            UnlinkedInstructionKind::MakeArray { dst, elements } => InstructionKind::MakeArray {
                dst: *dst,
                elements: elements.clone(),
            },
            UnlinkedInstructionKind::MakeTuple { dst, elements } => InstructionKind::MakeTuple {
                dst: *dst,
                elements: elements.clone(),
            },
            UnlinkedInstructionKind::MakeSetFromArray { dst, src } => {
                InstructionKind::MakeSetFromArray {
                    dst: *dst,
                    src: *src,
                }
            }
            UnlinkedInstructionKind::FormatString { dst, parts } => InstructionKind::FormatString {
                dst: *dst,
                parts: parts.clone(),
            },
            UnlinkedInstructionKind::MakeMap { dst, entries } => {
                let entries = entries
                    .iter()
                    .map(|(key, value)| {
                        let key = linked_code.push_constant(Constant::String(key.clone()));
                        (key, *value)
                    })
                    .collect();
                InstructionKind::MakeMap { dst: *dst, entries }
            }
            UnlinkedInstructionKind::MakeRange {
                dst,
                start,
                end,
                inclusive,
            } => InstructionKind::MakeRange {
                dst: *dst,
                start: *start,
                end: *end,
                inclusive: *inclusive,
            },
            UnlinkedInstructionKind::MakeRecord {
                dst,
                type_name,
                type_id,
                fields,
            } => {
                let ty = self.link_type(type_name, *type_id)?;
                let field_slots = sorted_field_slots(fields.iter().map(|(field, _)| field));
                let fields = fields
                    .iter()
                    .map(|(field, register)| {
                        (
                            FieldSlot::new(field_slots[field]),
                            self.linked.intern_debug_name(field.clone()),
                            *register,
                        )
                    })
                    .collect();
                InstructionKind::MakeRecord {
                    dst: *dst,
                    ty,
                    fields,
                }
            }
            UnlinkedInstructionKind::MakeEnum {
                dst,
                enum_name,
                type_id,
                variant,
                variant_id,
                fields,
            } => {
                let enum_ty = self.link_type(enum_name, *type_id)?;
                let variant = self.link_variant(enum_name, variant, *variant_id, enum_ty)?;
                let field_slots = sorted_field_slots(fields.iter().map(|(field, _)| field));
                let fields = fields
                    .iter()
                    .map(|(field, register)| {
                        (
                            FieldSlot::new(field_slots[field]),
                            self.linked.intern_debug_name(field.clone()),
                            *register,
                        )
                    })
                    .collect();
                InstructionKind::MakeEnum {
                    dst: *dst,
                    enum_ty,
                    variant,
                    fields,
                }
            }
            UnlinkedInstructionKind::GetRecordField { dst, record, field } => {
                InstructionKind::GetRecordField {
                    dst: *dst,
                    record: *record,
                    debug_name: self.linked.intern_debug_name(field.clone()),
                }
            }
            UnlinkedInstructionKind::GetRecordSlot {
                dst,
                record,
                field,
                slot,
            } => InstructionKind::GetRecordSlot {
                dst: *dst,
                record: *record,
                field: FieldSlot::new(*slot),
                debug_name: self.linked.intern_debug_name(field.clone()),
                cache_site: cache_site_at(code, instruction_offset),
            },
            UnlinkedInstructionKind::SetRecordField { record, field, src } => {
                InstructionKind::SetRecordField {
                    record: *record,
                    debug_name: self.linked.intern_debug_name(field.clone()),
                    src: *src,
                }
            }
            UnlinkedInstructionKind::SetRecordSlot {
                record,
                field,
                slot,
                src,
            } => InstructionKind::SetRecordSlot {
                record: *record,
                field: FieldSlot::new(*slot),
                debug_name: self.linked.intern_debug_name(field.clone()),
                cache_site: cache_site_at(code, instruction_offset),
                src: *src,
            },
            UnlinkedInstructionKind::GetEnumField { dst, value, field } => {
                InstructionKind::GetEnumField {
                    dst: *dst,
                    value: *value,
                    debug_name: self.linked.intern_debug_name(field.clone()),
                }
            }
            UnlinkedInstructionKind::GetEnumSlot {
                dst,
                value,
                field,
                slot,
            } => InstructionKind::GetEnumSlot {
                dst: *dst,
                value: *value,
                field: FieldSlot::new(*slot),
                debug_name: self.linked.intern_debug_name(field.clone()),
            },
            UnlinkedInstructionKind::TupleArityEqual { dst, value, arity } => {
                InstructionKind::TupleArityEqual {
                    dst: *dst,
                    value: *value,
                    arity: *arity,
                }
            }
            UnlinkedInstructionKind::GuardTupleArity { value, arity } => {
                InstructionKind::GuardTupleArity {
                    value: *value,
                    arity: *arity,
                }
            }
            UnlinkedInstructionKind::GetTupleField { dst, value, index } => {
                InstructionKind::GetTupleField {
                    dst: *dst,
                    value: *value,
                    index: *index,
                }
            }
            UnlinkedInstructionKind::GetIndex { dst, base, index } => InstructionKind::GetIndex {
                dst: *dst,
                base: *base,
                index: *index,
            },
            UnlinkedInstructionKind::GetStringKeyIndex { dst, base, key } => {
                InstructionKind::GetStringKeyIndex {
                    dst: *dst,
                    base: *base,
                    key: *key,
                }
            }
            UnlinkedInstructionKind::SetIndex { base, index, src } => InstructionKind::SetIndex {
                base: *base,
                index: *index,
                src: *src,
            },
            UnlinkedInstructionKind::SetStringKeyIndex { base, key, src } => {
                InstructionKind::SetStringKeyIndex {
                    base: *base,
                    key: *key,
                    src: *src,
                }
            }
            UnlinkedInstructionKind::IterInit { dst, iterable } => InstructionKind::IterInit {
                dst: *dst,
                iterable: *iterable,
            },
            UnlinkedInstructionKind::IterNext {
                iterator,
                dst,
                jump_if_done,
            } => InstructionKind::IterNext {
                iterator: *iterator,
                dst: *dst,
                jump_if_done: *jump_if_done,
            },
            UnlinkedInstructionKind::RangeNext {
                cursor,
                end,
                done,
                inclusive,
                dst,
                jump_if_done,
            } => InstructionKind::RangeNext {
                cursor: *cursor,
                end: *end,
                done: *done,
                inclusive: *inclusive,
                dst: *dst,
                jump_if_done: *jump_if_done,
            },
            UnlinkedInstructionKind::I64RangeNext {
                cursor,
                end,
                done,
                inclusive,
                dst,
                jump_if_done,
            } => InstructionKind::I64RangeNext {
                cursor: *cursor,
                end: *end,
                done: *done,
                inclusive: *inclusive,
                dst: *dst,
                jump_if_done: *jump_if_done,
            },
            UnlinkedInstructionKind::EnumTagEqual {
                dst,
                value,
                enum_name,
                type_id,
                variant,
                variant_id,
            } => {
                let enum_ty = self.link_type(enum_name, *type_id)?;
                let variant = self.link_variant(enum_name, variant, *variant_id, enum_ty)?;
                InstructionKind::EnumTagEqual {
                    dst: *dst,
                    value: *value,
                    enum_ty,
                    variant,
                }
            }
            UnlinkedInstructionKind::LoadState {
                dst,
                state,
                slot,
                cache_site,
            } => {
                let slot = slot.or_else(|| program.state_slot(state)).ok_or_else(|| {
                    LinkError::MissingState {
                        function: code.name.clone(),
                        state: state.clone(),
                    }
                })?;
                let debug_name = self.linked.intern_debug_name(state.clone());
                InstructionKind::LoadState {
                    dst: *dst,
                    slot,
                    debug_name,
                    cache_site: *cache_site,
                }
            }
            UnlinkedInstructionKind::StoreState { state, slot, src } => {
                let slot = slot.or_else(|| program.state_slot(state)).ok_or_else(|| {
                    LinkError::MissingState {
                        function: code.name.clone(),
                        state: state.clone(),
                    }
                })?;
                let debug_name = self.linked.intern_debug_name(state.clone());
                InstructionKind::StoreState {
                    slot,
                    debug_name,
                    src: *src,
                }
            }
            UnlinkedInstructionKind::LoadExternState {
                dst,
                state,
                slot,
                cache_site,
            } => {
                let slot = slot.or_else(|| program.state_slot(state)).ok_or_else(|| {
                    LinkError::MissingState {
                        function: code.name.clone(),
                        state: state.clone(),
                    }
                })?;
                let debug_name = self.linked.intern_debug_name(state.clone());
                InstructionKind::LoadExternState {
                    dst: *dst,
                    slot,
                    debug_name,
                    cache_site: *cache_site,
                }
            }
            UnlinkedInstructionKind::ReleaseBorrowLease { dst, src } => {
                InstructionKind::ReleaseBorrowLease {
                    dst: *dst,
                    src: *src,
                }
            }
            UnlinkedInstructionKind::TryReleaseBorrowLease { dst, src } => {
                InstructionKind::TryReleaseBorrowLease {
                    dst: *dst,
                    src: *src,
                }
            }
            UnlinkedInstructionKind::HostRead {
                dst,
                root,
                target,
                dynamic_args,
                cache_site,
            } => {
                let target = self.link_host_target(code, host_target_map, *target)?;
                InstructionKind::HostRead {
                    dst: *dst,
                    root: *root,
                    target,
                    dynamic_args: dynamic_args.clone(),
                    cache_site: *cache_site,
                }
            }
            UnlinkedInstructionKind::HostWrite {
                root,
                target,
                dynamic_args,
                src,
                cache_site,
            } => {
                let target = self.link_host_target(code, host_target_map, *target)?;
                InstructionKind::HostWrite {
                    root: *root,
                    target,
                    dynamic_args: dynamic_args.clone(),
                    src: *src,
                    cache_site: *cache_site,
                }
            }
            UnlinkedInstructionKind::HostMutate {
                root,
                target,
                dynamic_args,
                op,
                rhs,
                cache_site,
            } => {
                let target = self.link_host_target(code, host_target_map, *target)?;
                InstructionKind::HostMutate {
                    root: *root,
                    target,
                    dynamic_args: dynamic_args.clone(),
                    op: *op,
                    rhs: *rhs,
                    cache_site: *cache_site,
                }
            }
            UnlinkedInstructionKind::HostRemove {
                root,
                target,
                dynamic_args,
                cache_site,
            } => {
                let target = self.link_host_target(code, host_target_map, *target)?;
                InstructionKind::HostRemove {
                    root: *root,
                    target,
                    dynamic_args: dynamic_args.clone(),
                    cache_site: *cache_site,
                }
            }
            UnlinkedInstructionKind::HostCall {
                dst,
                root,
                target,
                dynamic_args,
                method,
                args,
                cache_site,
            } => {
                let target = self.link_host_target(code, host_target_map, *target)?;
                let dispatch = self.link_host_method(*method);
                let debug_text = format!("host_method::{}", method.get());
                let debug_name = self.linked.intern_debug_name(debug_text);
                InstructionKind::HostCall {
                    dst: *dst,
                    root: *root,
                    target,
                    dynamic_args: dynamic_args.clone(),
                    method: dispatch,
                    debug_name,
                    args: args.clone(),
                    cache_site: *cache_site,
                }
            }
            UnlinkedInstructionKind::GuardType { src, guard } => {
                let guard = self.link_type_guard(guard.clone(), linked_code)?;
                InstructionKind::GuardType { src: *src, guard }
            }
            UnlinkedInstructionKind::Return { src } => InstructionKind::Return { src: *src },
        };

        if let Some(cache_site) = instruction
            .kind
            .cache_site()
            .or_else(|| cache_site_at(code, instruction_offset))
        {
            kind.set_cache_site(cache_site);
        }

        Ok(Instruction {
            kind,
            span: instruction.span,
            execution_units: instruction.execution_units,
            mir_origin: instruction.mir_origin,
            mir_budget_charges: instruction.mir_budget_charges.clone(),
        })
    }
}

/// Resolves an integer literal's source text to its magnitude.
///
/// The interpreter used to run this per execution, allocating for the
/// underscore strip each time. Radix detection and underscore handling match
/// that earlier textual parse exactly so accepted programs keep their results
/// and rejected literals keep their runtime type error.
fn link_int_literal(literal: &str) -> crate::linked::LinkedIntLiteral {
    let text = literal.replace('_', "");
    let (digits, radix) = if let Some(digits) = text.strip_prefix("0x") {
        (digits, 16)
    } else if let Some(digits) = text.strip_prefix("0X") {
        (digits, 16)
    } else if let Some(digits) = text.strip_prefix("0b") {
        (digits, 2)
    } else if let Some(digits) = text.strip_prefix("0B") {
        (digits, 2)
    } else {
        (text.as_str(), 10)
    };
    u64::from_str_radix(digits, radix).map_or(
        crate::linked::LinkedIntLiteral::Unrepresentable,
        crate::linked::LinkedIntLiteral::Magnitude,
    )
}

/// Resolves a float literal's source text to its `f32` and `f64` values.
///
/// Unlike integer literals, the earlier textual parse did not strip
/// underscores, so this does not either.
fn link_float_literal(literal: &str) -> crate::linked::LinkedFloatLiteral {
    match (literal.parse::<f32>(), literal.parse::<f64>()) {
        (Ok(as_f32), Ok(as_f64)) => crate::linked::LinkedFloatLiteral::Value { as_f32, as_f64 },
        _ => crate::linked::LinkedFloatLiteral::Unrepresentable,
    }
}

#[cfg(test)]
mod tests;
