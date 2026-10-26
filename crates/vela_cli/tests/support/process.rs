use std::io::{self, Read};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(30);
const OUTPUT_LIMIT: usize = 1024 * 1024;

// Like the LSP stdio tests, own the child before starting either pipe reader.
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

pub fn run(command: &mut Command) -> Output {
    run_bounded(command, DEADLINE, OUTPUT_LIMIT).expect("bounded child process")
}

fn run_bounded(command: &mut Command, deadline: Duration, limit: usize) -> Result<Output, String> {
    let mut child = OwnedChild(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("spawn: {error}"))?,
    );
    capture(&mut child.0, deadline, limit)
}

fn reader(pipe: impl Read + Send + 'static, limit: usize) -> Receiver<io::Result<Vec<u8>>> {
    let (send, receive) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = pipe.take((limit + 1) as u64).read_to_end(&mut bytes);
        let _ = send.send(result.map(|_| bytes));
    });
    receive
}

fn poll(
    receiver: &Receiver<io::Result<Vec<u8>>>,
    bytes: &mut Option<Vec<u8>>,
    limit: usize,
    stream: &str,
) -> Result<(), String> {
    if bytes.is_some() {
        return Ok(());
    }
    match receiver.try_recv() {
        Ok(Ok(output)) if output.len() > limit => Err(format!("{stream} exceeded {limit} bytes")),
        Ok(Ok(output)) => {
            *bytes = Some(output);
            Ok(())
        }
        Ok(Err(error)) => Err(format!("{stream}: {error}")),
        Err(TryRecvError::Empty) => Ok(()),
        Err(TryRecvError::Disconnected) => Err(format!("{stream} reader disconnected")),
    }
}

fn capture(child: &mut Child, deadline: Duration, limit: usize) -> Result<Output, String> {
    let stdout = reader(child.stdout.take().expect("stdout pipe"), limit);
    let stderr = reader(child.stderr.take().expect("stderr pipe"), limit);
    let start = Instant::now();
    let mut out = None;
    let mut err = None;
    let result = (|| {
        loop {
            // Check both caps while the child is alive, not only after exit.
            poll(&stdout, &mut out, limit, "stdout")?;
            poll(&stderr, &mut err, limit, "stderr")?;
            if let Some(status) = child.try_wait().map_err(|error| error.to_string())?
                && out.is_some()
                && err.is_some()
            {
                return Ok(Output {
                    status,
                    stdout: out.take().expect("stdout"),
                    stderr: err.take().expect("stderr"),
                });
            }
            if start.elapsed() >= deadline {
                return Err(format!("child deadline exceeded ({deadline:?})"));
            }
            thread::sleep(Duration::from_millis(5));
        }
    })();
    if result.is_err() {
        let _ = child.kill();
        child
            .wait()
            .map_err(|error| format!("reap child: {error}"))?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::io::Write;

    // Spawn this test executable, never a shell or a CLI-only testing command.
    #[test]
    fn process_probe() {
        let Ok(mode) = std::env::var("VELA_PROCESS_PROBE") else {
            return;
        };
        match mode.as_str() {
            "timeout" => thread::sleep(Duration::from_secs(60)),
            "stdout" | "stderr" => {
                let bytes = vec![b'x'; 8192];
                if mode == "stdout" {
                    let _ = io::stdout().write_all(&bytes);
                } else {
                    let _ = io::stderr().write_all(&bytes);
                }
                thread::sleep(Duration::from_secs(60));
            }
            "both" => {
                io::stdout()
                    .write_all(&vec![b'o'; 128 * 1024])
                    .expect("stdout");
                io::stderr()
                    .write_all(&vec![b'e'; 128 * 1024])
                    .expect("stderr");
                std::process::exit(7);
            }
            _ => panic!("unknown probe"),
        }
    }

    fn probe(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().expect("test executable"));
        command
            .args([
                "--exact",
                "support::process::tests::process_probe",
                "--nocapture",
            ])
            .env("VELA_PROCESS_PROBE", mode);
        command
    }

    #[rstest]
    #[case::deadline("timeout", "deadline")]
    #[case::stdout_cap("stdout", "stdout exceeded")]
    #[case::stderr_cap("stderr", "stderr exceeded")]
    fn bounded_failure_kills_and_reaps(#[case] mode: &str, #[case] expected: &str) {
        let mut child = OwnedChild(
            probe(mode)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("probe"),
        );
        let start = Instant::now();
        let error = capture(&mut child.0, Duration::from_secs(2), 1024).expect_err("must reject");
        assert!(error.contains(expected), "{error}");
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(child.0.try_wait().expect("already reaped").is_some());
    }

    #[test]
    fn captures_both_streams_beyond_pipe_capacity_and_exact_status() {
        let output = run_bounded(&mut probe("both"), Duration::from_secs(10), OUTPUT_LIMIT)
            .expect("both pipes drained");
        assert_eq!(output.status.code(), Some(7));
        assert!(output.stdout.ends_with(&vec![b'o'; 128 * 1024]));
        assert_eq!(output.stderr, vec![b'e'; 128 * 1024]);
    }
}
