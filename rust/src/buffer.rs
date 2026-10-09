//! Bounded reusable I/O. Output flushing is explicit; buffering is not durability.
use serde::Serialize;
use std::io::{self, BufRead, BufWriter, Write};

pub struct Lines<R> {
    reader: R,
    bytes: Vec<u8>,
    limit: usize,
    physical_line: usize,
    failed: bool,
}
impl<R: BufRead> Lines<R> {
    pub fn new(reader: R, limit: usize) -> io::Result<Self> {
        if limit == 0 || limit > 16 * 1024 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid_line_limit",
            ));
        }
        Ok(Self {
            reader,
            bytes: Vec::with_capacity(limit.min(8192)),
            limit,
            physical_line: 0,
            failed: false,
        })
    }
    /// Borrows the same allocation across calls; oversized input is terminal to callers.
    pub fn next_line(&mut self) -> io::Result<Option<(usize, &[u8])>> {
        if self.failed {
            return Err(io::Error::other("input_stream_failed"));
        }
        self.bytes.clear();
        loop {
            let chunk = match self.reader.fill_buf() {
                Ok(chunk) => chunk,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    self.failed = true;
                    return Err(e);
                }
            };
            if chunk.is_empty() {
                if self.bytes.is_empty() {
                    return Ok(None);
                }
                break;
            }
            let end = chunk.iter().position(|&x| x == b'\n').map(|i| i + 1);
            let n = end.unwrap_or(chunk.len());
            if n > self.limit - self.bytes.len() {
                self.failed = true;
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "input_line_too_large",
                ));
            }
            self.bytes.extend_from_slice(&chunk[..n]);
            self.reader.consume(n);
            if end.is_some() {
                break;
            }
        }
        self.physical_line += 1;
        Ok(Some((self.physical_line, &self.bytes)))
    }
}

pub struct JsonLines<W: Write> {
    writer: Option<BufWriter<W>>,
    scratch: Vec<u8>,
    flush_each: bool,
    limit: usize,
    failed: bool,
}
struct Limited<'a> {
    bytes: &'a mut Vec<u8>,
    limit: usize,
}
impl Write for Limited<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "output_record_too_large",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl<W: Write> JsonLines<W> {
    pub fn new(writer: W, flush_each: bool) -> Self {
        Self::with_limit(writer, flush_each, 1_048_576).expect("valid_default_output_limit")
    }
    /// The limit includes the final newline. Serialization errors commit no bytes.
    pub fn with_limit(writer: W, flush_each: bool, limit: usize) -> io::Result<Self> {
        if limit == 0 || limit > 16 * 1024 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid_output_limit",
            ));
        }
        Ok(Self {
            writer: Some(BufWriter::with_capacity(64 * 1024, writer)),
            scratch: Vec::with_capacity(1024),
            flush_each,
            limit,
            failed: false,
        })
    }
    pub fn record(&mut self, value: &impl Serialize) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::other("output_stream_failed"));
        }
        self.scratch.clear();
        serde_json::to_writer(
            Limited {
                bytes: &mut self.scratch,
                limit: self.limit - 1,
            },
            value,
        )
        .map_err(io::Error::other)?;
        self.scratch.push(b'\n');
        let writer = self.writer.as_mut().expect("healthy_writer");
        if let Err(error) = writer.write_all(&self.scratch).and_then(|()| {
            if self.flush_each {
                writer.flush()
            } else {
                Ok(())
            }
        }) {
            self.poison();
            return Err(error);
        }
        Ok(())
    }
    pub fn finish(&mut self) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::other("output_stream_failed"));
        }
        match self.writer.as_mut().expect("healthy_writer").flush() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.poison();
                Err(error)
            }
        }
    }
    fn poison(&mut self) {
        self.failed = true;
        if let Some(writer) = self.writer.take() {
            let _ = writer.into_parts();
        }
    }
}
