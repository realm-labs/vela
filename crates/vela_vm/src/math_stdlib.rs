use crate::owned_value::OwnedValue;
use crate::{Vm, VmError, VmErrorKind, VmResult};
use vela_common::standard_ids::{
    MATH_ABS_FUNCTION_ID, MATH_CEIL_FUNCTION_ID, MATH_CLAMP_FUNCTION_ID,
    MATH_DISTANCE2D_FUNCTION_ID, MATH_DISTANCE3D_FUNCTION_ID, MATH_FLOOR_FUNCTION_ID,
    MATH_LERP_FUNCTION_ID, MATH_MAX_FUNCTION_ID, MATH_MIN_FUNCTION_ID,
    MATH_MOVE_TOWARDS_FUNCTION_ID, MATH_POW_FUNCTION_ID, MATH_ROUND_FUNCTION_ID,
    MATH_SIGN_FUNCTION_ID, MATH_SQRT_FUNCTION_ID,
};

mod distance;
mod movement;
mod power;
mod root;
mod scalar;

use distance::{math_distance2d, math_distance3d};
use movement::{math_lerp, math_move_towards};
use power::math_pow;
use root::math_sqrt;
use scalar::{
    math_abs, math_ceil, math_clamp, math_floor, math_max, math_min, math_round, math_sign,
};

pub(crate) fn register(vm: &mut Vm) {
    vm.register_native_with_id(MATH_MAX_FUNCTION_ID, "math::max", math_max);
    vm.register_native_with_id(MATH_MIN_FUNCTION_ID, "math::min", math_min);
    vm.register_native_with_id(MATH_CLAMP_FUNCTION_ID, "math::clamp", math_clamp);
    vm.register_native_with_id(MATH_LERP_FUNCTION_ID, "math::lerp", math_lerp);
    vm.register_native_with_id(
        MATH_MOVE_TOWARDS_FUNCTION_ID,
        "math::move_towards",
        math_move_towards,
    );
    vm.register_native_with_id(
        MATH_DISTANCE2D_FUNCTION_ID,
        "math::distance2d",
        math_distance2d,
    );
    vm.register_native_with_id(
        MATH_DISTANCE3D_FUNCTION_ID,
        "math::distance3d",
        math_distance3d,
    );
    vm.register_native_with_id(MATH_POW_FUNCTION_ID, "math::pow", math_pow);
    vm.register_native_with_id(MATH_SQRT_FUNCTION_ID, "math::sqrt", math_sqrt);
    vm.register_native_with_id(MATH_SIGN_FUNCTION_ID, "math::sign", math_sign);
    vm.register_native_with_id(MATH_FLOOR_FUNCTION_ID, "math::floor", math_floor);
    vm.register_native_with_id(MATH_CEIL_FUNCTION_ID, "math::ceil", math_ceil);
    vm.register_native_with_id(MATH_ROUND_FUNCTION_ID, "math::round", math_round);
    vm.register_native_with_id(MATH_ABS_FUNCTION_ID, "math::abs", math_abs);
}

pub(super) fn expect_finite_float(value: &OwnedValue, operation: &'static str) -> VmResult<f64> {
    match value {
        OwnedValue::Int(value) => Ok(*value as f64),
        OwnedValue::Float(value) if value.is_finite() => Ok(*value),
        _ => type_error(operation),
    }
}

pub(super) fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError::new(VmErrorKind::TypeMismatch { operation }))
}

pub(super) fn expect_arity(name: &str, args: &[OwnedValue], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError::new(VmErrorKind::ArityMismatch {
        name: name.to_owned(),
        expected,
        actual: args.len(),
    }))
}

#[cfg(test)]
mod tests {
    use vela_bytecode::compiler::compile_function_source;
    use vela_common::SourceId;

    use crate::{ExecutionBudget, OwnedValue, Vm};

    #[test]
    fn runs_compiled_math_distance2d() {
        let source = r#"
fn main() {
    let distance = math::distance2d(0, 0, 3, 4);
    if distance == 5.0 && math::distance2d(-1.5, 2.0, -1.5, 5.0) == 3.0 {
        return math::round(distance);
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math distance2d source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("math distance2d should run");
        assert_eq!(result, OwnedValue::Int(5));
    }

    #[test]
    fn runs_compiled_math_distance3d() {
        let source = r#"
fn main() {
    let distance = math::distance3d(0, 0, 0, 2, 3, 6);
    if distance == 7.0 && math::distance3d(-1.5, 2.0, 4.0, -1.5, 5.0, 8.0) == 5.0 {
        return math::round(distance);
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math distance3d source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("math distance3d should run");
        assert_eq!(result, OwnedValue::Int(7));
    }

    #[test]
    fn runs_compiled_math_pow() {
        let source = r#"
fn main() {
    if math::pow(2, 10) == 1024 && math::pow(9, 0.5) == 3.0 && math::sqrt(81) == 9.0 {
        return math::pow(2, 3);
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math pow source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("math pow should run");
        assert_eq!(result, OwnedValue::Int(8));
    }

    #[test]
    fn runs_compiled_math_sqrt() {
        let source = r#"
fn main() {
    if math::sqrt(49) == 7.0 && math::sqrt(2.25) == 1.5 {
        return math::round(math::sqrt(16));
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math sqrt source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("math sqrt should run");
        assert_eq!(result, OwnedValue::Int(4));
    }

    #[test]
    fn runs_compiled_math_sign() {
        let source = r#"
fn main() {
    return math::sign(-12)
        + math::sign(0)
        + math::sign(3.5)
        + math::sign(-0.0);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math sign source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("math sign should run");
        assert_eq!(result, OwnedValue::Int(0));
    }

    #[test]
    fn runs_compiled_math_move_towards() {
        let source = r#"
fn main() {
    let forward = math::move_towards(0, 10, 3);
    let snapped = math::move_towards(8, 10, 5);
    let backward = math::move_towards(10, 0, 4);
    let float_step = math::move_towards(1.5, 4.0, 1.25);
    if float_step == 2.75 {
        return forward + snapped + backward;
    }
    return 0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math move_towards source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let result = vm.run(&code).expect("math move_towards should run");
        assert_eq!(result, OwnedValue::Int(19));
    }

    #[test]
    fn managed_heap_execution_runs_math_distance2d() {
        let source = r#"
fn main() {
    return math::distance2d(2, 4, 8, 12) == 10.0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap math distance2d source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap math distance2d should run");
        assert_eq!(result, OwnedValue::Bool(true));
    }

    #[test]
    fn managed_heap_execution_runs_math_distance3d() {
        let source = r#"
fn main() {
    return math::distance3d(1, 2, 3, 4, 6, 15) == 13.0;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap math distance3d source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap math distance3d should run");
        assert_eq!(result, OwnedValue::Bool(true));
    }

    #[test]
    fn managed_heap_execution_runs_math_pow() {
        let source = r#"
fn main() {
    return math::pow(16, 0.5) == 4.0 && math::pow(3, 4) == 81;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap math pow source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap math pow should run");
        assert_eq!(result, OwnedValue::Bool(true));
    }

    #[test]
    fn managed_heap_execution_runs_math_sqrt() {
        let source = r#"
fn main() {
    return math::sqrt(64) == 8.0 && math::sqrt(0.25) == 0.5;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap math sqrt source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap math sqrt should run");
        assert_eq!(result, OwnedValue::Bool(true));
    }

    #[test]
    fn managed_heap_execution_runs_math_sign() {
        let source = r#"
fn main() {
    return math::sign(-2.5) == -1 && math::sign(0.0) == 0 && math::sign(8) == 1;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap math sign source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap math sign should run");
        assert_eq!(result, OwnedValue::Bool(true));
    }

    #[test]
    fn managed_heap_execution_runs_math_move_towards() {
        let source = r#"
fn main() {
    return math::move_towards(0, 10, 0) == 0
        && math::move_towards(0.0, -2.0, 0.5) == -0.5
        && math::move_towards(5, 2, 10) == 2;
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("heap math move_towards source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();
        let mut budget = ExecutionBudget::unbounded();

        let result = vm
            .run_with_managed_heap_and_budget(&code, &mut budget)
            .expect("heap math move_towards should run");
        assert_eq!(result, OwnedValue::Bool(true));
    }

    #[test]
    fn math_distance2d_rejects_non_numeric_values() {
        let source = r#"
fn main() {
    return math::distance2d(0, 0, "x", 1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math distance2d type error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math distance2d should reject non-numeric values");
        assert_eq!(
            error.kind(),
            crate::VmErrorKind::TypeMismatch {
                operation: "math::distance2d"
            }
        );
    }

    #[test]
    fn math_pow_rejects_non_numeric_values() {
        let source = r#"
fn main() {
    return math::pow("xp", 2);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math pow type error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math pow should reject non-numeric values");
        assert_eq!(
            error.kind(),
            crate::VmErrorKind::TypeMismatch {
                operation: "math::pow"
            }
        );
    }

    #[test]
    fn math_sqrt_rejects_negative_values() {
        let source = r#"
fn main() {
    return math::sqrt(-1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math sqrt negative source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math sqrt should reject negative values");
        assert_eq!(
            error.kind(),
            crate::VmErrorKind::TypeMismatch {
                operation: "math::sqrt"
            }
        );
    }

    #[test]
    fn math_sqrt_rejects_non_numeric_values() {
        let source = r#"
fn main() {
    return math::sqrt("xp");
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math sqrt type error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math sqrt should reject non-numeric values");
        assert_eq!(
            error.kind(),
            crate::VmErrorKind::TypeMismatch {
                operation: "math::sqrt"
            }
        );
    }

    #[test]
    fn math_sign_rejects_non_numeric_values() {
        let source = r#"
fn main() {
    return math::sign("left");
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math sign type error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math sign should reject non-numeric values");
        assert_eq!(
            error.kind(),
            crate::VmErrorKind::TypeMismatch {
                operation: "math::sign"
            }
        );
    }

    #[test]
    fn math_move_towards_rejects_negative_delta() {
        let source = r#"
fn main() {
    return math::move_towards(0, 10, -1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math move_towards negative delta source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math move_towards should reject negative max_delta");
        assert_eq!(
            error.kind(),
            crate::VmErrorKind::TypeMismatch {
                operation: "math::move_towards"
            }
        );
    }

    #[test]
    fn math_lerp_rejects_non_finite_results() {
        let source = r#"
fn main() {
    return math::lerp(1.0e308, -1.0e308, 2.0);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math lerp non-finite source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math lerp should reject non-finite results");
        assert_eq!(
            error.kind(),
            crate::VmErrorKind::TypeMismatch {
                operation: "math::lerp"
            }
        );
    }

    #[test]
    fn math_move_towards_rejects_non_numeric_values() {
        let source = r#"
fn main() {
    return math::move_towards(0, "target", 1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math move_towards type error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math move_towards should reject non-numeric values");
        assert_eq!(
            error.kind(),
            crate::VmErrorKind::TypeMismatch {
                operation: "math::move_towards"
            }
        );
    }

    #[test]
    fn math_pow_rejects_non_finite_results() {
        let source = r#"
fn main() {
    return math::pow(0, -1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math pow non-finite source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math pow should reject non-finite results");
        assert_eq!(
            error.kind(),
            crate::VmErrorKind::TypeMismatch {
                operation: "math::pow"
            }
        );
    }

    #[test]
    fn math_distance3d_rejects_non_numeric_values() {
        let source = r#"
fn main() {
    return math::distance3d(0, 0, 0, 1, "y", 1);
}
"#;
        let code = compile_function_source(SourceId::new(1), source, "main")
            .expect("math distance3d type error source should compile");
        let mut vm = Vm::new();
        vm.register_standard_natives();

        let error = vm
            .run(&code)
            .expect_err("math distance3d should reject non-numeric values");
        assert_eq!(
            error.kind(),
            crate::VmErrorKind::TypeMismatch {
                operation: "math::distance3d"
            }
        );
    }
}
