//! Bounded capture for optional runtime discovery and execution.

use std::io::{self, Read};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const OUTPUT_LIMIT: u64 = 64 * 1024;

pub(crate) fn output(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = capture(child.stdout.take().expect("piped stdout"));
    let stderr = capture(child.stderr.take().expect("piped stderr"));
    let deadline = Instant::now() + timeout;
    let result = (|| {
        let mut status = None;
        let mut out = None;
        let mut err = None;
        loop {
            receive(&stdout, &mut out)?;
            receive(&stderr, &mut err)?;
            if status.is_none() {
                status = child.try_wait()?;
            }
            if let Some(status) = status
                && out.is_some()
                && err.is_some()
            {
                return Ok(Output {
                    status,
                    stdout: out.take().expect("captured stdout"),
                    stderr: err.take().expect("captured stderr"),
                });
            }
            // Do not wait indefinitely on inherited pipes after the child exits.
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "runtime process deadline exceeded",
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

fn capture(reader: impl Read + Send + 'static) -> mpsc::Receiver<io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = reader
            .take(OUTPUT_LIMIT + 1)
            .read_to_end(&mut bytes)
            .and_then(|_| {
                if bytes.len() as u64 > OUTPUT_LIMIT {
                    Err(io::Error::other(
                        "runtime output exceeds 64 KiB stream limit",
                    ))
                } else {
                    Ok(bytes)
                }
            });
        let _ = sender.send(result);
    });
    receiver
}

fn receive(
    receiver: &mpsc::Receiver<io::Result<Vec<u8>>>,
    result: &mut Option<Vec<u8>>,
) -> io::Result<()> {
    if result.is_none() {
        match receiver.try_recv() {
            Ok(bytes) => *result = Some(bytes?),
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err(io::Error::other("runtime output reader disconnected"));
            }
        }
    }
    Ok(())
}
