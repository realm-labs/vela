use std::collections::{BTreeMap, BTreeSet};

use vela_common::{Diagnostic, Span};
use vela_hir::binding::LocalBindingKind;
use vela_hir::body::{
    HirAssignOp, HirBinaryOp, HirBodyRoot, HirElseBranch, HirExprKind, HirFloatSuffix, HirIf,
    HirIntegerSuffix, HirLiteral, HirMatch, HirMatchArmBody, HirPathKind, HirPatternKind,
    HirStmtKind, HirUnaryOp,
};
use vela_hir::ids::{HirBlockId, HirBodyId, HirExprId, HirPatternId, HirStmtId};
use vela_hir::type_hint::ParamHint;
use vela_host::resolved::HostMutationOp;
use vela_syntax::token::{InterpolatedStringTokenPart, TokenKind};

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

mod assignments;
mod calls;
mod control_flow;
mod operators;
mod values;

fn integer_text(value: &vela_hir::body::HirIntegerLiteral) -> String {
    let mut text = value.text.clone();
    if let Some(suffix) = value.suffix {
        text.push_str(match suffix {
            HirIntegerSuffix::I8 => "i8",
            HirIntegerSuffix::I16 => "i16",
            HirIntegerSuffix::I32 => "i32",
            HirIntegerSuffix::I64 => "i64",
            HirIntegerSuffix::U8 => "u8",
            HirIntegerSuffix::U16 => "u16",
            HirIntegerSuffix::U32 => "u32",
            HirIntegerSuffix::U64 => "u64",
        });
    }
    text
}

fn float_text(value: &vela_hir::body::HirFloatLiteral) -> String {
    let mut text = value.text.clone();
    if let Some(suffix) = value.suffix {
        text.push_str(match suffix {
            HirFloatSuffix::F32 => "f32",
            HirFloatSuffix::F64 => "f64",
        });
    }
    text
}

pub(in crate::compiler) fn integer_suffix_tag(
    suffix: Option<HirIntegerSuffix>,
) -> vela_common::PrimitiveTag {
    match suffix {
        None | Some(HirIntegerSuffix::I64) => vela_common::PrimitiveTag::I64,
        Some(HirIntegerSuffix::I8) => vela_common::PrimitiveTag::I8,
        Some(HirIntegerSuffix::I16) => vela_common::PrimitiveTag::I16,
        Some(HirIntegerSuffix::I32) => vela_common::PrimitiveTag::I32,
        Some(HirIntegerSuffix::U8) => vela_common::PrimitiveTag::U8,
        Some(HirIntegerSuffix::U16) => vela_common::PrimitiveTag::U16,
        Some(HirIntegerSuffix::U32) => vela_common::PrimitiveTag::U32,
        Some(HirIntegerSuffix::U64) => vela_common::PrimitiveTag::U64,
    }
}

pub(in crate::compiler) fn float_suffix_tag(
    suffix: Option<HirFloatSuffix>,
) -> vela_common::PrimitiveTag {
    match suffix {
        Some(HirFloatSuffix::F32) => vela_common::PrimitiveTag::F32,
        None | Some(HirFloatSuffix::F64) => vela_common::PrimitiveTag::F64,
    }
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
