use std::path::{Path, PathBuf};
use std::process::Command;

fn run_demo(script: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_vela_cli"))
        .arg(script_path(script))
        .output()
        .expect("run vela_cli demo script");

    assert!(
        output.status.success(),
        "demo script failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    String::from_utf8(output.stdout).expect("demo stdout should be utf8")
}

fn script_path(script: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/game_server_demo/scripts")
        .join(script)
}

#[test]
fn level_up_demo_runs_through_cli() {
    assert_eq!(
        run_demo("level_up.lang"),
        "result=Int(10) level=Int(10) patches=1\n"
    );
}

#[test]
fn context_event_demo_runs_through_cli() {
    assert_eq!(
        run_demo("context_event.lang"),
        "result=Int(1700000042) level=Int(9) ctx_now=Int(1700000000) \
         ctx_tick=Int(42) emits=1 patches=1\n"
    );
}

#[test]
fn monster_kill_reward_demo_runs_through_cli() {
    assert_eq!(
        run_demo("monster_kill_reward.lang"),
        "result=Int(2) level=Int(2) exp=Int(0) quest_count=Int(2) \
         quest_done=Bool(false) rewards=1 emits=2 patches=6\n"
    );
}

#[test]
fn quest_progress_demo_runs_through_cli() {
    assert_eq!(
        run_demo("quest_progress.lang"),
        "result=Int(3) level=Int(1) exp=Int(90) quest_count=Int(3) \
         quest_done=Bool(true) rewards=0 emits=1 patches=3\n"
    );
}
