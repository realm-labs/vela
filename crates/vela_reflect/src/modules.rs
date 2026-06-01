pub mod descriptors;
mod queries;
mod records;
mod script;

pub use descriptors::{
    DeclOrigin, FunctionDesc, FunctionParamDesc, ModuleDesc, ModuleExportDesc, ModuleExportKind,
};
pub use queries::{
    callable_function_name_with_policy, exports, exports_for_target,
    exports_for_target_with_policy, exports_with_policy, function, function_with_policy, functions,
    functions_with_policy, has_function, has_function_with_policy, has_module,
    has_module_with_policy, module, module_with_policy, modules, modules_with_policy,
};

#[cfg(test)]
mod tests;
