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
    pub fn script(callee: CompileCalleeTarget, arguments: Vec<CompileScriptCallArgument>) -> Self {
        Self {
            callee,
            arguments: CompileCallArguments::Script(arguments),
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
    /// Complete parameter slots for a script callee. A missing value is valid
    /// only when that parameter owns a HIR default body.
    Script(Vec<CompileScriptCallArgument>),
    /// Already-validated positional order for native, stdlib, host, value,
    /// callable-value, and behavior-intrinsic calls.
    Positional(Vec<HirExprId>),
    /// Runtime positional/named order for a genuinely dynamic call boundary.
    Dynamic(Vec<CompileDynamicCallArgument>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileScriptCallArgument {
    pub parameter: u32,
    pub value: Option<HirExprId>,
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
