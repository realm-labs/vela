use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::call_args::{HirCallArgument, resolve_hir_call_arguments};
use crate::compiler::calls::metadata::{registry_param_hints, unresolved_static_method_error};
use crate::compiler::calls::{mutation_arg_debug_name, typed_container_mutation_arg_contract};
use crate::compiler::constructors::schema_default_fields;
use crate::compiler::control_flow::{LoopContext, LoopIterable};
use crate::compiler::expected_exprs::guard_location_and_name;
use crate::compiler::host_paths::HostIndexAccessKind;
use crate::compiler::patterns::PatternBindingFacts;
use crate::compiler::patterns::enum_variant_path;
use crate::compiler::record_shapes::{ValueShape, callback_param_shapes};
use crate::compiler::schema_defaults::{
    ConstructorFieldUse, record_constructor_field_diagnostics, unknown_enum_variant_diagnostic,
};
use crate::compiler::value_types::{
    ExpectedTypeOutcome, RuntimeTypeFact, StaticExprType, TypeContractContext, check_expected_type,
};
use crate::compiler::{CompileError, CompileErrorKind, CompileResult, Compiler, frame_slot_kind};
use crate::{
    BinaryLiteralSide, CallArgument, Constant, DynamicCallArgument, FormatStringPart, GuardKind,
    Register, ScriptCallMode, UnlinkedGuardContext, UnlinkedInstructionKind, UnlinkedTypeGuard,
};
use vela_common::{Diagnostic, Span};
use vela_hir::binding::LocalBindingKind;
use vela_hir::body::{
    HirAssignOp, HirBinaryOp, HirBodyRoot, HirElseBranch, HirExprKind, HirFloatSuffix, HirIf,
    HirIntegerSuffix, HirInterpolatedStringPart, HirLiteral, HirMatch, HirMatchArmBody,
    HirPathKind, HirPatternKind, HirStmtKind, HirUnaryOp,
};
use vela_hir::ids::{HirBlockId, HirBodyId, HirExprId, HirPatternId, HirStmtId};
use vela_hir::type_hint::ParamHint;
use vela_host::resolved::HostMutationOp;

mod assignments;
mod calls;
mod control_flow;
mod operators;
mod values;

fn integer_text(value: &vela_hir::body::HirIntegerLiteral) -> String {
    vela_analysis::literals::integer_literal_spelling(value)
}

fn float_text(value: &vela_hir::body::HirFloatLiteral) -> String {
    vela_analysis::literals::float_literal_spelling(value)
}

pub(in crate::compiler) fn integer_suffix_tag(
    suffix: Option<HirIntegerSuffix>,
) -> vela_common::PrimitiveTag {
    vela_analysis::literals::integer_suffix_primitive(suffix)
}

pub(in crate::compiler) fn float_suffix_tag(
    suffix: Option<HirFloatSuffix>,
) -> vela_common::PrimitiveTag {
    vela_analysis::literals::float_suffix_primitive(suffix)
}

fn hir_host_mutation_op(op: HirAssignOp) -> Option<HostMutationOp> {
    match op {
        HirAssignOp::Set => None,
        HirAssignOp::Add => Some(HostMutationOp::Add),
        HirAssignOp::Sub => Some(HostMutationOp::Sub),
        HirAssignOp::Mul => Some(HostMutationOp::Mul),
        HirAssignOp::Div => Some(HostMutationOp::Div),
        HirAssignOp::Rem => Some(HostMutationOp::Rem),
    }
}

fn hir_compound_instruction(
    op: HirAssignOp,
    dst: Register,
    lhs: Register,
    rhs: Register,
    i64_specialized: bool,
) -> Option<UnlinkedInstructionKind> {
    if i64_specialized {
        let specialized = match op {
            HirAssignOp::Add => Some(UnlinkedInstructionKind::I64Add { dst, lhs, rhs }),
            HirAssignOp::Sub => Some(UnlinkedInstructionKind::I64Sub { dst, lhs, rhs }),
            HirAssignOp::Mul => Some(UnlinkedInstructionKind::I64Mul { dst, lhs, rhs }),
            HirAssignOp::Rem => None,
            HirAssignOp::Set | HirAssignOp::Div => None,
        };
        if specialized.is_some() {
            return specialized;
        }
    }
    match op {
        HirAssignOp::Set => None,
        HirAssignOp::Add => Some(UnlinkedInstructionKind::Add { dst, lhs, rhs }),
        HirAssignOp::Sub => Some(UnlinkedInstructionKind::Sub { dst, lhs, rhs }),
        HirAssignOp::Mul => Some(UnlinkedInstructionKind::Mul { dst, lhs, rhs }),
        HirAssignOp::Div => Some(UnlinkedInstructionKind::Div { dst, lhs, rhs }),
        HirAssignOp::Rem => Some(UnlinkedInstructionKind::Rem { dst, lhs, rhs }),
    }
}

fn hir_binary_op_name(op: HirBinaryOp) -> &'static str {
    match op {
        HirBinaryOp::IdentityEqual => "===",
        HirBinaryOp::IdentityNotEqual => "!==",
        _ => "binary operator",
    }
}

fn hir_unsupported(feature: &'static str, span: Span) -> CompileError {
    CompileError::new(CompileErrorKind::UnsupportedSyntax(feature)).with_span(span)
}
