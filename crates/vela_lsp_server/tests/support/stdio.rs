use crossbeam_channel::bounded;
use lsp_server::Message;
use serde_json::Value;
use std::io::{BufReader, Cursor, Read, Seek, Write};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(10);
const OUTPUT_LIMIT: usize = 1024 * 1024;
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
pub fn run(input: Vec<Vec<u8>>) -> Output {
    let mut child = OwnedChild(
        Command::new(env!("CARGO_BIN_EXE_vela_lsp_server"))
            .arg("--stdio")
            .arg("--no-watch-files")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start owned server process"),
    );
    let mut stdin = child.0.stdin.take().expect("stdin");
    // The parent enforces the deadline while a writer may be blocked in OS IO.
    // Closing/killing the owned child releases all pipes on failure.
    let (written, writes) = bounded(1);
    thread::spawn(move || {
        let result = input
            .into_iter()
            .try_for_each(|chunk| stdin.write_all(&chunk));
        drop(stdin);
        let _ = written.send(result);
    });
    let stdout = child.0.stdout.take().expect("stdout");
    let stderr = child.0.stderr.take().expect("stderr");
    let (out, outs) = bounded(1);
    let (err, errs) = bounded(1);
    thread::spawn(move || {
        let mut bytes = vec![];
        let result = stdout
            .take((OUTPUT_LIMIT + 1) as u64)
            .read_to_end(&mut bytes);
        let _ = out.send(result.map(|_| bytes));
    });
    thread::spawn(move || {
        let mut bytes = vec![];
        let result = stderr
            .take((OUTPUT_LIMIT + 1) as u64)
            .read_to_end(&mut bytes);
        let _ = err.send(result.map(|_| bytes));
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait().expect("poll owned child") {
            break status;
        }
        assert!(
            start.elapsed() < DEADLINE,
            "stdio process must terminate within ten seconds"
        );
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = outs
        .recv_timeout(DEADLINE)
        .expect("stdout drain deadline")
        .expect("stdout read");
    let stderr = errs
        .recv_timeout(DEADLINE)
        .expect("stderr drain deadline")
        .expect("stderr read");
    assert!(
        stdout.len() <= OUTPUT_LIMIT && stderr.len() <= OUTPUT_LIMIT,
        "bounded process output"
    );
    // A rejected transport may close its input before the finite writer ends.
    let written = writes.recv_timeout(DEADLINE).expect("writer deadline");
    if status.success() {
        written.expect("complete successful conversation write");
    }
    Output {
        status,
        stdout,
        stderr,
    }
}
pub fn frame(value: Value) -> Vec<u8> {
    let message: Message = serde_json::from_value(value).expect("typed input envelope");
    let mut bytes = vec![];
    message.write(&mut bytes).expect("typed frame");
    bytes
}
pub fn messages(bytes: &[u8]) -> Vec<Value> {
    let mut reader = BufReader::new(Cursor::new(bytes));
    let mut messages = vec![];
    loop {
        let start = reader.stream_position().expect("frame start") as usize;
        if Message::read(&mut reader)
            .expect("only complete typed frames on stdout")
            .is_none()
        {
            break;
        }
        let end = reader.stream_position().expect("frame end") as usize;
        let frame = &bytes[start..end];
        let body = frame
            .windows(4)
            .position(|part| part == b"\r\n\r\n")
            .expect("header terminator")
            + 4;
        let value: Value = serde_json::from_slice(&frame[body..]).expect("original wire JSON");
        assert_eq!(
            value["jsonrpc"], "2.0",
            "do not synthesize lost wire fields"
        );
        messages.push(value);
    }
    messages
}
