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

#[test]
fn game_server_examples_run() {
    let cases = [
        (
            env!("CARGO_BIN_EXE_level_up"),
            "result=Int(10) level=Int(10)\n",
        ),
        (
            env!("CARGO_BIN_EXE_context_event"),
            "result=Int(1700000042) level=Int(9) ctx_now=Int(1700000000) \
             ctx_tick=Int(42) emits=1 logs=1\n",
        ),
        (
            env!("CARGO_BIN_EXE_time_clock"),
            "result=Int(52) level=Int(9)\n",
        ),
        (
            env!("CARGO_BIN_EXE_gameplay_helpers"),
            "result=Int(9) level=Int(9)\n",
        ),
        (
            env!("CARGO_BIN_EXE_random_allowed"),
            "result=Int(310) level=Int(9)\n",
        ),
        (
            env!("CARGO_BIN_EXE_random_reflect_allowed"),
            "result=Int(310) level=Int(9)\n",
        ),
        (
            env!("CARGO_BIN_EXE_reward_preview"),
            "result=Int(22) level=Int(1)\n",
        ),
        (
            env!("CARGO_BIN_EXE_monster_kill_reward"),
            "result=Int(2) level=Int(2) exp=Int(0) quest_count=Int(3) \
             quest_done=Bool(true) inventory_gold=Int(2) reward_calls=1 emits=3\n",
        ),
        (
            env!("CARGO_BIN_EXE_quest_progress"),
            "result=Int(3) level=Int(1) exp=Int(90) quest_count=Int(3) \
             quest_done=Bool(true) inventory_gold=Int(0) reward_calls=0 emits=1\n",
        ),
        (
            env!("CARGO_BIN_EXE_reflect_debug"),
            "result=Int(22) level=Int(12) ctx_now=Int(1700000000) \
             ctx_tick=Int(42) emits=1\n",
        ),
        (
            env!("CARGO_BIN_EXE_host_type_methods"),
            "script_result=Int(10) final_count=10 score=7 reward_calls=3\n",
        ),
        (env!("CARGO_BIN_EXE_modules"), "module_result=Int(16)\n"),
        (
            env!("CARGO_BIN_EXE_native_function"),
            "native_function_result=Int(45) final_level=16\n",
        ),
        (
            env!("CARGO_BIN_EXE_script_global"),
            "script_global first=Int(9) second=Int(27) name=String(\"rust-updated\") \
             projected=Int(31) final_level=Int(11) final_gold=Int(8) ticks=Int(8)\n",
        ),
    ];

    for (bin, expected) in cases {
        assert_eq!(run_bin(bin), expected);
    }
}

#[test]
fn expected_error_examples_run() {
    let cases = [
        (
            env!("CARGO_BIN_EXE_random_permission_denied"),
            "native `math::random` requires capability `random`",
        ),
        (
            env!("CARGO_BIN_EXE_host_read_only_denied"),
            "field `Player.id` is read-only for script writes",
        ),
        (
            env!("CARGO_BIN_EXE_host_permission_denied"),
            "action: \"read\"",
        ),
        (
            env!("CARGO_BIN_EXE_host_write_permission_denied"),
            "action: \"write\"",
        ),
        (
            env!("CARGO_BIN_EXE_host_call_permission_denied"),
            "action: \"call\"",
        ),
        (
            env!("CARGO_BIN_EXE_host_compound_write_denied"),
            "action: \"write\"",
        ),
        (env!("CARGO_BIN_EXE_stale_host_ref"), "StaleGeneration"),
        (
            env!("CARGO_BIN_EXE_bad_schema_duplicate_field"),
            "duplicate field `item_id`",
        ),
        (
            env!("CARGO_BIN_EXE_generic_type_hint_denied"),
            "script type hints do not support generics",
        ),
        (
            env!("CARGO_BIN_EXE_reflect_schema_mutation_denied"),
            "invalid reflection target",
        ),
        (
            env!("CARGO_BIN_EXE_reflect_unknown_field_denied"),
            "unknown reflected field `leve`",
        ),
        (
            env!("CARGO_BIN_EXE_hot_reload_function_swap_invalid"),
            "hot reload rejected: v0 unchanged",
        ),
    ];

    for (bin, expected) in cases {
        assert!(
            run_bin(bin).contains(expected),
            "example `{bin}` should contain `{expected}`"
        );
    }
}

#[test]
fn hot_reload_example_runs() {
    assert_eq!(
        run_bin(env!("CARGO_BIN_EXE_hot_reload_function_swap")),
        "hot reload accepted: v0 -> v1\n\
         changed functions: kill_exp\n\
         safe_point=tick_boundary abi=checked old_version=0 new_version=1 \
         old_before=Int(20) old_after=Int(20) new_after=Int(30)\n"
    );
}
