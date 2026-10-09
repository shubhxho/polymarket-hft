//! Bounded reusable I/O. Output flushing is explicit; buffering is not durability.
use serde::Serialize;
use std::io::{self, BufRead, BufWriter, Write};

pub struct Lines<R> {
    reader: R,
    bytes: Vec<u8>,
    limit: usize,
    physical_line: usize,
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
        })
    }
    /// Borrows the same allocation across calls; oversized input is terminal to callers.
    pub fn next_line(&mut self) -> io::Result<Option<(usize, &[u8])>> {
        self.bytes.clear();
        loop {
            let chunk = self.reader.fill_buf()?;
            if chunk.is_empty() {
                if self.bytes.is_empty() {
                    return Ok(None);
                }
                break;
            }
            let end = chunk.iter().position(|&x| x == b'\n').map(|i| i + 1);
            let n = end.unwrap_or(chunk.len());
            if n > self.limit - self.bytes.len() {
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
    writer: BufWriter<W>,
    scratch: Vec<u8>,
    flush_each: bool,
}
impl<W: Write> JsonLines<W> {
    pub fn new(writer: W, flush_each: bool) -> Self {
        Self {
            writer: BufWriter::with_capacity(64 * 1024, writer),
            scratch: Vec::with_capacity(1024),
            flush_each,
        }
    }
    pub fn record(&mut self, value: &impl Serialize) -> io::Result<()> {
        self.scratch.clear();
        serde_json::to_writer(&mut self.scratch, value).map_err(io::Error::other)?;
        self.scratch.push(b'\n');
        self.writer.write_all(&self.scratch)?;
        if self.flush_each {
            self.writer.flush()?;
        }
        Ok(())
    }
    pub fn finish(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
