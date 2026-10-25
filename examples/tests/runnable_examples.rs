use std::process::Command;

fn run_bin(path: &str) -> String {
    let output = Command::new(path).output().expect("run example bin");
    assert!(
        output.status.success(),
        "example failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("example stdout should be utf8")
}

macro_rules! success_example {
    ($name:ident, $bin:expr, $expected:expr $(,)?) => {
        #[test]
        fn $name() {
            assert_eq!(run_bin($bin), $expected);
        }
    };
}

macro_rules! output_contains_example {
    ($name:ident, $bin:expr, $expected:expr $(,)?) => {
        #[test]
        fn $name() {
            let bin = $bin;
            let expected = $expected;
            assert!(
                run_bin(bin).contains(expected),
                "example `{bin}` should contain `{expected}`"
            );
        }
    };
}

success_example!(
    modules_example_runs,
    env!("CARGO_BIN_EXE_modules"),
    "module_result=Scalar(I64(16))\n",
);

success_example!(
    native_function_example_runs,
    env!("CARGO_BIN_EXE_native_function"),
    "native_function_result=Scalar(I64(44))\n",
);

success_example!(
    io_stdlib_example_runs,
    env!("CARGO_BIN_EXE_io_stdlib"),
    "hello from fs\nio_stdlib len=Scalar(I64(13)) output=done\n",
);

success_example!(
    script_state_example_runs,
    env!("CARGO_BIN_EXE_script_state"),
    "script_state first=9 second=27 name=rust-updated \
     projected=31 final_level=11 final_gold=8 ticks=8\n",
);

success_example!(
    serde_value_example_runs,
    env!("CARGO_BIN_EXE_serde_value"),
    "serde_value actor=player-1001 applied=34 score=39 label=slash original_amount=9\n",
);

success_example!(
    container_type_hints_example_runs,
    env!("CARGO_BIN_EXE_container_type_hints"),
    "container_type_hints result=Scalar(I64(17))\n",
);

success_example!(
    hot_reload_example_runs,
    env!("CARGO_BIN_EXE_hot_reload_function_swap"),
    "hot reload accepted: v0 -> v1\n\
         changed functions: kill_exp\n\
         safe_point=tick_boundary abi=checked old_version=0 new_version=1 \
         old_before=Scalar(I64(20)) old_after=Scalar(I64(20)) \
         new_after=Scalar(I64(30))\n",
);

output_contains_example!(
    random_permission_denied_example_reports_error,
    env!("CARGO_BIN_EXE_random_permission_denied"),
    "native `math::random` requires capability `random`",
);

output_contains_example!(
    bad_schema_duplicate_field_example_reports_error,
    env!("CARGO_BIN_EXE_bad_schema_duplicate_field"),
    "duplicate field `item_id`",
);

output_contains_example!(
    generic_type_hint_denied_example_reports_error,
    env!("CARGO_BIN_EXE_generic_type_hint_denied"),
    "only builtin container, Option, and Result type hints support type arguments",
);

output_contains_example!(
    reflect_schema_mutation_denied_example_reports_error,
    env!("CARGO_BIN_EXE_reflect_schema_mutation_denied"),
    "invalid reflection target",
);

output_contains_example!(
    reflect_unknown_field_denied_example_reports_error,
    env!("CARGO_BIN_EXE_reflect_unknown_field_denied"),
    "unknown reflected field `leve`",
);

output_contains_example!(
    hot_reload_function_swap_invalid_example_reports_error,
    env!("CARGO_BIN_EXE_hot_reload_function_swap_invalid"),
    "hot reload rejected: v0 unchanged",
);
