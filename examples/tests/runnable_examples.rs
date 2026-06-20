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
    level_up_example_runs,
    env!("CARGO_BIN_EXE_level_up"),
    "result=Scalar(I64(10)) level=Scalar(I64(10))\n",
);

success_example!(
    context_event_example_runs,
    env!("CARGO_BIN_EXE_context_event"),
    "result=Scalar(I64(1700000042)) level=Scalar(I64(9)) \
     ctx_now=Scalar(I64(1700000000)) ctx_tick=Scalar(I64(42)) emits=1 logs=1\n",
);

success_example!(
    time_clock_example_runs,
    env!("CARGO_BIN_EXE_time_clock"),
    "result=Scalar(I64(52)) level=Scalar(I64(9))\n",
);

success_example!(
    gameplay_helpers_example_runs,
    env!("CARGO_BIN_EXE_gameplay_helpers"),
    "result=Scalar(I64(9)) level=Scalar(I64(9))\n",
);

success_example!(
    random_allowed_example_runs,
    env!("CARGO_BIN_EXE_random_allowed"),
    "result=Scalar(I64(310)) level=Scalar(I64(9))\n",
);

success_example!(
    random_reflect_allowed_example_runs,
    env!("CARGO_BIN_EXE_random_reflect_allowed"),
    "result=Scalar(I64(310)) level=Scalar(I64(9))\n",
);

success_example!(
    reward_preview_example_runs,
    env!("CARGO_BIN_EXE_reward_preview"),
    "result=Scalar(I64(22)) level=Scalar(I64(1))\n",
);

success_example!(
    monster_kill_reward_example_runs,
    env!("CARGO_BIN_EXE_monster_kill_reward"),
    "result=Scalar(I64(2)) level=Scalar(I64(2)) exp=Scalar(I64(0)) \
     quest_count=Scalar(I64(3)) quest_done=Bool(true) \
     inventory_gold=Scalar(I64(2)) reward_calls=1 emits=3\n",
);

success_example!(
    quest_progress_example_runs,
    env!("CARGO_BIN_EXE_quest_progress"),
    "result=Scalar(I64(3)) level=Scalar(I64(1)) exp=Scalar(I64(90)) \
     quest_count=Scalar(I64(3)) quest_done=Bool(true) \
     inventory_gold=Scalar(I64(0)) reward_calls=0 emits=1\n",
);

success_example!(
    reflect_debug_example_runs,
    env!("CARGO_BIN_EXE_reflect_debug"),
    "result=Scalar(I64(22)) level=Scalar(I64(12)) \
     ctx_now=Scalar(I64(1700000000)) ctx_tick=Scalar(I64(42)) emits=1\n",
);

success_example!(
    host_type_methods_example_runs,
    env!("CARGO_BIN_EXE_host_type_methods"),
    "script_result=Scalar(I64(10)) final_count=10 score=7 reward_calls=3\n",
);

success_example!(
    modules_example_runs,
    env!("CARGO_BIN_EXE_modules"),
    "module_result=Scalar(I64(16))\n",
);

success_example!(
    native_function_example_runs,
    env!("CARGO_BIN_EXE_native_function"),
    "native_function_result=Scalar(I64(45)) final_level=16\n",
);

success_example!(
    io_stdlib_example_runs,
    env!("CARGO_BIN_EXE_io_stdlib"),
    "hello from fs\nio_stdlib len=Scalar(I64(13)) output=done\n",
);

success_example!(
    script_global_example_runs,
    env!("CARGO_BIN_EXE_script_global"),
    "script_global first=9 second=27 name=rust-updated \
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
    host_read_only_denied_example_reports_error,
    env!("CARGO_BIN_EXE_host_read_only_denied"),
    "field `Player.id` is read-only for script writes",
);

output_contains_example!(
    host_permission_denied_example_reports_error,
    env!("CARGO_BIN_EXE_host_permission_denied"),
    "action: \"read\"",
);

output_contains_example!(
    host_write_permission_denied_example_reports_error,
    env!("CARGO_BIN_EXE_host_write_permission_denied"),
    "action: \"write\"",
);

output_contains_example!(
    host_call_permission_denied_example_reports_error,
    env!("CARGO_BIN_EXE_host_call_permission_denied"),
    "action: \"call\"",
);

output_contains_example!(
    host_compound_write_denied_example_reports_error,
    env!("CARGO_BIN_EXE_host_compound_write_denied"),
    "action: \"write\"",
);

output_contains_example!(
    stale_host_ref_example_reports_error,
    env!("CARGO_BIN_EXE_stale_host_ref"),
    "StaleGeneration",
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
