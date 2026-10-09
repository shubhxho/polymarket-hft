use polymarket_hft::buffer::{JsonLines, Lines};
use std::io::{self, BufRead, Cursor, Read, Write};
#[test]
fn oversized_lines_are_terminal_and_interrupted_reads_retry() {
    let mut input = Lines::new(Cursor::new(b"toolong\nok\n"), 3).unwrap();
    assert!(input.next_line().is_err());
    assert_eq!(
        input.next_line().unwrap_err().to_string(),
        "input_stream_failed"
    );
    struct Once {
        inner: Cursor<&'static [u8]>,
        interrupted: bool,
    }
    impl Read for Once {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            self.inner.read(b)
        }
    }
    impl BufRead for Once {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            if !self.interrupted {
                self.interrupted = true;
                Err(io::Error::from(io::ErrorKind::Interrupted))
            } else {
                self.inner.fill_buf()
            }
        }
        fn consume(&mut self, n: usize) {
            self.inner.consume(n)
        }
    }
    let mut input = Lines::new(
        Once {
            inner: Cursor::new(b"ok\n"),
            interrupted: false,
        },
        3,
    )
    .unwrap();
    assert_eq!(input.next_line().unwrap().unwrap().1, b"ok\n");
}
#[test]
fn serialization_limits_include_newline_and_commit_no_partial_record() {
    let mut bytes = Vec::new();
    {
        let mut output = JsonLines::with_limit(&mut bytes, false, 2).unwrap();
        assert!(output.record(&"large").is_err());
        output.record(&1).unwrap();
        output.finish().unwrap();
    }
    assert_eq!(bytes, b"1\n");
    assert!(JsonLines::with_limit(Vec::new(), false, 0).is_err());
}
#[test]
fn broken_output_never_retries_even_on_drop() {
    use std::{cell::Cell, rc::Rc};
    struct Partial(Rc<Cell<usize>>);
    impl Write for Partial {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            let n = self.0.get();
            self.0.set(n + 1);
            if n == 0 {
                Ok(b.len().min(1))
            } else {
                Err(io::Error::other("broken_pipe"))
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let calls = Rc::new(Cell::new(0));
    {
        let mut output = JsonLines::new(Partial(calls.clone()), true);
        assert!(output.record(&123).is_err());
        assert_eq!(calls.get(), 2);
        assert!(output.record(&456).is_err());
        assert!(output.finish().is_err());
    }
    assert_eq!(calls.get(), 2);
}
