use std::sync::{Arc, Mutex};

use vela_common::FunctionId;
use vela_vm::error::{VmError, VmErrorKind, VmResult};
use vela_vm::owned_value::OwnedValue;

use crate::native::{
    EffectSet, FunctionAccess, NativeFunctionDesc, NativeFunctionEntry, NativeFunctionId, TypeHint,
};

pub const CONTROLLED_RANDOM_PERMISSION: &str = "std.random";
pub const MATH_RANDOM_FUNCTION_ID: NativeFunctionId = FunctionId::new(0xff00_0001);

pub(crate) fn controlled_math_random(seed: u64) -> NativeFunctionEntry {
    let rng = Arc::new(Mutex::new(SeededRandom::new(seed)));
    NativeFunctionEntry::new(
        NativeFunctionDesc::new("math::random", MATH_RANDOM_FUNCTION_ID)
            .param("min", TypeHint::Int)
            .param("max", TypeHint::Int)
            .returns(TypeHint::Int)
            .effects(EffectSet::pure())
            .access(
                FunctionAccess::public()
                    .reflect_callable(true)
                    .require_permission(CONTROLLED_RANDOM_PERMISSION),
            )
            .attr("stdlib", "math")
            .docs("Returns a deterministic seeded integer in the inclusive range."),
        move |args| math_random(args, &rng),
    )
}

fn math_random(args: &[OwnedValue], rng: &Mutex<SeededRandom>) -> VmResult<OwnedValue> {
    expect_arity("math::random", args, 2)?;
    let (OwnedValue::Int(min), OwnedValue::Int(max)) = (&args[0], &args[1]) else {
        return type_error("math::random");
    };
    if min > max {
        return type_error("math::random");
    }

    let range = u128::try_from(i128::from(*max) - i128::from(*min) + 1).map_err(|_| VmError {
        kind: VmErrorKind::TypeMismatch {
            operation: "math::random",
        },
        source_span: None,
        call_stack: Default::default(),
    })?;
    let mut rng = rng.lock().map_err(|_| VmError {
        kind: VmErrorKind::TypeMismatch {
            operation: "math::random",
        },
        source_span: None,
        call_stack: Default::default(),
    })?;
    let offset = u128::from(rng.next_u64()) % range;
    let value = i128::from(*min)
        + i128::try_from(offset).map_err(|_| VmError {
            kind: VmErrorKind::TypeMismatch {
                operation: "math::random",
            },
            source_span: None,
            call_stack: Default::default(),
        })?;
    i64::try_from(value)
        .map(OwnedValue::Int)
        .map_err(|_| VmError {
            kind: VmErrorKind::TypeMismatch {
                operation: "math::random",
            },
            source_span: None,
            call_stack: Default::default(),
        })
}

#[derive(Clone, Copy, Debug)]
struct SeededRandom {
    state: u64,
}

impl SeededRandom {
    const MULTIPLIER: u64 = 6_364_136_223_846_793_005;
    const INCREMENT: u64 = 1_442_695_040_888_963_407;

    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::INCREMENT);
        self.state
    }
}

fn type_error<T>(operation: &'static str) -> VmResult<T> {
    Err(VmError {
        kind: VmErrorKind::TypeMismatch { operation },
        source_span: None,
        call_stack: Default::default(),
    })
}

fn expect_arity(name: &str, args: &[OwnedValue], expected: usize) -> VmResult<()> {
    if args.len() == expected {
        return Ok(());
    }
    Err(VmError {
        kind: VmErrorKind::ArityMismatch {
            name: name.to_owned(),
            expected,
            actual: args.len(),
        },
        source_span: None,
        call_stack: Default::default(),
    })
}
