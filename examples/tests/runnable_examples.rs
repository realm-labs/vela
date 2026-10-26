mod support;

use rstest::rstest;
use std::process::Command;

fn run_bin(path: &str) -> String {
    let cwd = vela_examples::temp_dir::TempDir::new().expect("isolated example cwd");
    let output = support::process::run(Command::new(path).current_dir(cwd.path()));
    assert!(
        output.status.success(),
        "example failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("example stdout should be utf8")
}

#[rstest]
#[case::modules(
    env!("CARGO_BIN_EXE_modules"),
    "module_result=Scalar(I64(16))\n",
)]
#[case::native_function(
    env!("CARGO_BIN_EXE_native_function"),
    "native_function_result=Scalar(I64(44))\n",
)]
#[case::io_stdlib(
    env!("CARGO_BIN_EXE_io_stdlib"),
    "hello from fs\nio_stdlib len=Scalar(I64(13)) output=done\n",
)]
#[case::script_state(
    env!("CARGO_BIN_EXE_script_state"),
    "script_state first=9 second=27 name=rust-updated \
     projected=31 final_level=11 final_gold=8 ticks=8\n",
)]
#[case::serde_value(
    env!("CARGO_BIN_EXE_serde_value"),
    "serde_value actor=player-1001 applied=34 score=39 label=slash original_amount=9\n",
)]
#[case::container_type_hints(
    env!("CARGO_BIN_EXE_container_type_hints"),
    "container_type_hints result=Scalar(I64(17))\n",
)]
#[case::hot_reload_function_swap(
    env!("CARGO_BIN_EXE_hot_reload_function_swap"),
    "hot reload accepted: v0 -> v1\n\
         changed functions: kill_exp\n\
         safe_point=tick_boundary abi=checked old_version=0 new_version=1 \
         old_before=Scalar(I64(20)) old_after=Scalar(I64(20)) \
         new_after=Scalar(I64(30))\n",
)]
#[case::package_dependency_demo(
    env!("CARGO_BIN_EXE_package_dependency_demo"),
    "ordinary package dependency reload: 42 -> 43\n",
)]
fn example_exact_output(#[case] bin: &str, #[case] expected: &str) {
    assert_eq!(run_bin(bin), expected);
}

#[rstest]
#[case::random_permission_denied(
    env!("CARGO_BIN_EXE_random_permission_denied"),
    "native `math::random` requires capability `random`",
)]
#[case::bad_schema_duplicate_field(
    env!("CARGO_BIN_EXE_bad_schema_duplicate_field"),
    "duplicate field `item_id`",
)]
#[case::generic_type_hint_denied(
    env!("CARGO_BIN_EXE_generic_type_hint_denied"),
    "only builtin container, Option, and Result type hints support type arguments",
)]
#[case::reflect_schema_mutation_denied(
    env!("CARGO_BIN_EXE_reflect_schema_mutation_denied"),
    "invalid reflection target",
)]
#[case::reflect_unknown_field_denied(
    env!("CARGO_BIN_EXE_reflect_unknown_field_denied"),
    "unknown reflected field `leve`",
)]
#[case::hot_reload_function_swap_invalid(
    env!("CARGO_BIN_EXE_hot_reload_function_swap_invalid"),
    "hot reload rejected: v0 unchanged",
)]
fn example_contains_output(#[case] bin: &str, #[case] expected: &str) {
    assert!(
        run_bin(bin).contains(expected),
        "example `{bin}` should contain `{expected}`"
    );
}
