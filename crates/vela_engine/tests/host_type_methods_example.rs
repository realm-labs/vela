use std::path::Path;
use std::process::Command;

#[test]
fn host_type_methods_example_runs() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("engine crate should live under workspace/crates");
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args([
            "run",
            "-p",
            "vela_engine",
            "--example",
            "host_type_methods",
            "--quiet",
        ])
        .output()
        .expect("run host type methods example");

    assert!(
        output.status.success(),
        "example failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("example stdout should be utf8"),
        "script_result=Int(10) final_count=10 score=7 \
         reward_calls=3\n"
    );
}
