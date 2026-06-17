use std::io::{self, BufRead, Write};

use crate::{JsonRpcResult, LaunchConfiguration, LspServer};

pub fn run_stdio<R, W>(reader: R, writer: W) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    run_stdio_with_configuration(reader, writer, LaunchConfiguration::new())
}

pub fn run_stdio_with_configuration<R, W>(
    reader: R,
    writer: W,
    configuration: LaunchConfiguration,
) -> io::Result<()>
where
    R: BufRead,
    W: Write,
{
    let mut transport = StdioTransport::new(reader, writer);
    let mut server = LspServer::with_launch_configuration(configuration);
    while let Some(message) = transport.read_message()? {
        let result = server.handle_json(&message);
        transport.write_result(result)?;
        transport.flush()?;
        if server.is_exited() {
            break;
        }
    }
    transport.flush()
}

struct StdioTransport<R, W> {
    reader: R,
    writer: W,
}

impl<R, W> StdioTransport<R, W>
where
    R: BufRead,
    W: Write,
{
    fn new(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }

    fn read_message(&mut self) -> io::Result<Option<String>> {
        let Some(content_length) = self.read_content_length()? else {
            return Ok(None);
        };
        let mut body = vec![0_u8; content_length];
        self.reader.read_exact(&mut body)?;
        String::from_utf8(body)
            .map(Some)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn read_content_length(&mut self) -> io::Result<Option<usize>> {
        let mut content_length: Option<usize> = None;
        let mut line = String::new();
        loop {
            line.clear();
            let bytes = self.reader.read_line(&mut line)?;
            if bytes == 0 {
                return Ok(None);
            }
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                return content_length.map(Some).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length header")
                });
            }
            let Some((name, value)) = trimmed.split_once(':') else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid LSP header `{trimmed}`"),
                ));
            };
            if name.eq_ignore_ascii_case("Content-Length") {
                let length = value.trim().parse::<usize>().map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid Content-Length `{}`: {error}", value.trim()),
                    )
                })?;
                content_length = Some(length);
            }
        }
    }

    fn write_result(&mut self, result: JsonRpcResult) -> io::Result<()> {
        match result {
            JsonRpcResult::Response(message) | JsonRpcResult::Notification(message) => {
                self.write_message(&message)
            }
            JsonRpcResult::Notifications(messages) => {
                for message in messages {
                    self.write_message(&message)?;
                }
                Ok(())
            }
            JsonRpcResult::None => Ok(()),
        }
    }

    fn write_message(&mut self, message: &str) -> io::Result<()> {
        write!(
            self.writer,
            "Content-Length: {}\r\n\r\n{}",
            message.len(),
            message
        )
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Cursor, Write};

    #[test]
    fn stdio_flushes_after_each_response_before_stream_end() {
        let initialize = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "capabilities": {}
            }
        })
        .to_string();
        let input = format!("Content-Length: {}\r\n\r\n{initialize}", initialize.len());
        let mut writer = FlushCountingWriter::default();

        super::run_stdio(Cursor::new(input.into_bytes()), &mut writer)
            .expect("stdio transport should flush initialize response");

        assert!(
            writer.flush_count >= 2,
            "expected one flush after the response and one final flush, got {}",
            writer.flush_count
        );
        assert!(
            String::from_utf8_lossy(&writer.bytes).contains("\"id\":1"),
            "initialize response should be written"
        );
    }

    #[derive(Default)]
    struct FlushCountingWriter {
        bytes: Vec<u8>,
        flush_count: usize,
    }

    impl Write for FlushCountingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flush_count += 1;
            Ok(())
        }
    }
}
