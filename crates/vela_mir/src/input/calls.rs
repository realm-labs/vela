use vela_def::{FunctionId, MethodId, TypeId};
use vela_hir::ids::{HirBodyId, HirExprId, HirLocalId};

use super::{CompileHostPathTarget, DynamicMethodTarget, HostMethodTarget, MethodExecutableTarget};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileCallTarget {
    pub callee: CompileCalleeTarget,
    pub arguments: CompileCallArguments,
}

impl CompileCallTarget {
    #[must_use]
    pub fn script(
        callee: CompileCalleeTarget,
        evaluation_order: Vec<HirExprId>,
        parameter_slots: Vec<CompilePlacedCallArgument>,
    ) -> Self {
        Self {
            callee,
            arguments: CompileCallArguments::Script {
                evaluation_order,
                parameter_slots,
            },
        }
    }

    #[must_use]
    pub fn positional(callee: CompileCalleeTarget, arguments: Vec<HirExprId>) -> Self {
        Self {
            callee,
            arguments: CompileCallArguments::Positional(arguments),
        }
    }

    #[must_use]
    pub fn external_named(
        callee: CompileCalleeTarget,
        evaluation_order: Vec<HirExprId>,
        parameter_slots: Vec<CompilePlacedCallArgument>,
    ) -> Self {
        Self {
            callee,
            arguments: CompileCallArguments::ExternalNamed {
                evaluation_order,
                parameter_slots,
            },
        }
    }

    #[must_use]
    pub fn dynamic(
        callee: CompileCalleeTarget,
        arguments: Vec<CompileDynamicCallArgument>,
    ) -> Self {
        Self {
            callee,
            arguments: CompileCallArguments::Dynamic(arguments),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileCalleeTarget {
    ScriptFunction {
        function: FunctionId,
        debug_name: String,
    },
    ScriptMethod {
        target: MethodExecutableTarget,
        debug_name: String,
    },
    Local(HirLocalId),
    Lambda(HirBodyId),
    NativeFunction {
        function: FunctionId,
        debug_name: String,
    },
    StdlibFunction {
        function: FunctionId,
        debug_name: String,
    },
    ValueMethod {
        owner: TypeId,
        method: MethodId,
        debug_name: String,
    },
    HostMethod(HostMethodTarget),
    Reflection {
        operation: CompileReflectionCall,
        function: FunctionId,
        debug_name: String,
    },
    SetFromArray {
        function: FunctionId,
        debug_name: String,
    },
    HostRemove {
        path: CompileHostPathTarget,
    },
    HostPush {
        path: CompileHostPathTarget,
    },
    DynamicCallable,
    DynamicMethod(DynamicMethodTarget),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileCallArguments {
    /// Source evaluation order and complete parameter slots for a script
    /// callee. Explicit slots refer back to `evaluation_order` by source index;
    /// a missing slot is valid only when that parameter owns a HIR default
    /// body.
    Script {
        evaluation_order: Vec<HirExprId>,
        parameter_slots: Vec<CompilePlacedCallArgument>,
    },
    /// A signature-resolved external named call. Expressions remain in source
    /// evaluation order while complete parameter slots describe target
    /// placement and runtime-provided defaults.
    ExternalNamed {
        evaluation_order: Vec<HirExprId>,
        parameter_slots: Vec<CompilePlacedCallArgument>,
    },
    /// Already-validated positional order for native, stdlib, host, value,
    /// callable-value, and behavior-intrinsic calls.
    Positional(Vec<HirExprId>),
    /// Runtime positional/named order for a genuinely dynamic call boundary.
    Dynamic(Vec<CompileDynamicCallArgument>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilePlacedCallArgument {
    pub parameter: u32,
    pub value: CompilePlacedCallValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilePlacedCallValue {
    Explicit { source_index: u32, value: HirExprId },
    MissingDefault,
}

impl CompilePlacedCallArgument {
    #[must_use]
    pub const fn placed(parameter: u32, source_index: u32, value: HirExprId) -> Self {
        Self {
            parameter,
            value: CompilePlacedCallValue::Explicit {
                source_index,
                value,
            },
        }
    }

    #[must_use]
    pub const fn missing(parameter: u32) -> Self {
        Self {
            parameter,
            value: CompilePlacedCallValue::MissingDefault,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileDynamicCallArgument {
    pub name: Option<String>,
    pub value: HirExprId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileReflectionCall {
    Read,
    Write,
    Call,
}
