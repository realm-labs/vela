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
