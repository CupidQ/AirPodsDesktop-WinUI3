//! Bounded diagnostic log reads for the frontend's background worker.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub const LOG_LINE_LIMIT: usize = 200;
pub const LOG_BYTE_LIMIT: usize = 256 * 1024;
const NEWLINE: &str = if cfg!(windows) { "\r\n" } else { "\n" };

/// Read the last 200 lines using at most 256 KiB of input. Oversized trailing
/// lines are marked as truncated; appends after opening the snapshot are ignored.
pub fn read_log_tail(path: &Path) -> io::Result<String> {
    read_tail(&mut File::open(path)?, LOG_LINE_LIMIT, LOG_BYTE_LIMIT)
}

fn read_tail(
    reader: &mut (impl Read + Seek),
    lines: usize,
    max_bytes: usize,
) -> io::Result<String> {
    if lines == 0 || max_bytes == 0 {
        return Ok(String::new());
    }
    let length = reader.seek(SeekFrom::End(0))?;
    let byte_count = length.min(max_bytes as u64) as usize;
    let offset = length - byte_count as u64;
    reader.seek(SeekFrom::Start(offset))?;
    // Take only the captured length: a live writer cannot make this read unbounded.
    let mut bytes = Vec::with_capacity(byte_count);
    reader.take(byte_count as u64).read_to_end(&mut bytes)?;

    let mut start = suffix_start(&bytes, lines);
    let truncated = offset > 0 && start == 0;
    if truncated {
        // A byte cap can cut through a UTF-8 character; skip continuation bytes.
        while start < bytes.len() && bytes[start] & 0xc0 == 0x80 {
            start += 1;
        }
    }
    let text = String::from_utf8_lossy(&bytes[start..]);
    let text = if offset == 0 && start == 0 {
        text.strip_prefix('\u{feff}').unwrap_or(&text)
    } else {
        &text
    };
    // Match StreamReader's LF/CRLF/CR line semantics; keep embedded NULs safe
    // for the existing caller-owned C string ABI.
    let normalized = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\0', "�");
    let result = normalized
        .split_terminator('\n')
        .collect::<Vec<_>>()
        .join(NEWLINE);
    if truncated {
        Ok(format!("（日志内容过长，仅显示末尾片段）{NEWLINE}{result}"))
    } else {
        Ok(result)
    }
}

fn suffix_start(bytes: &[u8], lines: usize) -> usize {
    let mut cursor = bytes.len();
    // A terminal delimiter ends the last line, rather than adding a new one.
    if cursor > 0 && bytes[cursor - 1] == b'\n' {
        cursor -= 1;
        if cursor > 0 && bytes[cursor - 1] == b'\r' {
            cursor -= 1;
        }
    } else if cursor > 0 && bytes[cursor - 1] == b'\r' {
        cursor -= 1;
    }
    let mut boundaries = 0;
    while cursor > 0 {
        cursor -= 1;
        if matches!(bytes[cursor], b'\n' | b'\r') {
            boundaries += 1;
            if boundaries == lines {
                return cursor + 1;
            }
            if bytes[cursor] == b'\n' && cursor > 0 && bytes[cursor - 1] == b'\r' {
                cursor -= 1;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn line_endings_and_empty_lines_match_stream_reader() {
        for separator in ["\n", "\r\n", "\r"] {
            for trailing in [false, true] {
                let mut input = ["一", "", "三", "四"].join(separator);
                if trailing {
                    input.push_str(separator);
                }
                assert_eq!(
                    read_tail(&mut Cursor::new(input), 3, 1024).unwrap(),
                    ["", "三", "四"].join(NEWLINE)
                );
            }
        }
        for input in ["", "\n", "\r\n", "\r"] {
            assert_eq!(read_tail(&mut Cursor::new(input), 200, 1024).unwrap(), "");
        }
        assert_eq!(
            read_tail(&mut Cursor::new("a\n\n"), 2, 1024).unwrap(),
            ["a", ""].join(NEWLINE)
        );
    }

    #[test]
    fn unicode_bom_and_single_line_are_preserved() {
        let text = "\u{feff}中文 🙂\n第二行\n";
        assert_eq!(
            read_tail(&mut Cursor::new(text), 200, 1024).unwrap(),
            ["中文 🙂", "第二行"].join(NEWLINE)
        );
        assert_eq!(
            read_tail(&mut Cursor::new("single"), 1, 1024).unwrap(),
            "single"
        );
    }

    struct CountingReader {
        inner: Cursor<Vec<u8>>,
        bytes_read: usize,
    }
    impl Read for CountingReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let count = self.inner.read(buffer)?;
            self.bytes_read += count;
            Ok(count)
        }
    }
    impl Seek for CountingReader {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            self.inner.seek(position)
        }
    }

    #[test]
    fn large_logs_read_only_a_bounded_tail() {
        let mut input = vec![b'x'; 4 * 1024 * 1024];
        input.push(b'\n');
        for i in 0..1000 {
            input.extend_from_slice(format!("中文 line {i}\n").as_bytes());
        }
        let mut reader = CountingReader {
            inner: Cursor::new(input),
            bytes_read: 0,
        };
        let expected = (800..1000)
            .map(|i| format!("中文 line {i}"))
            .collect::<Vec<_>>()
            .join(NEWLINE);
        assert_eq!(
            read_tail(&mut reader, LOG_LINE_LIMIT, LOG_BYTE_LIMIT).unwrap(),
            expected
        );
        assert_eq!(reader.bytes_read, LOG_BYTE_LIMIT);
    }

    #[test]
    fn huge_lines_are_bounded_and_marked_without_broken_utf8() {
        let input = "🙂".repeat(1000);
        let output = read_tail(&mut Cursor::new(input), 200, 17).unwrap();
        assert!(output.starts_with("（日志内容过长，仅显示末尾片段）"));
        assert!(output.ends_with(&"🙂".repeat(4)));
        assert!(!output.contains('�'));
    }

    #[test]
    fn invalid_utf8_and_nul_do_not_break_the_string_abi() {
        let output = read_tail(&mut Cursor::new(b"ok\0\xff\n"), 200, 1024).unwrap();
        assert_eq!(output, "ok��");
        assert!(!output.contains('\0'));
    }

    #[test]
    fn zero_limits_return_empty_without_reading() {
        assert_eq!(read_tail(&mut Cursor::new("data"), 0, 1024).unwrap(), "");
        assert_eq!(read_tail(&mut Cursor::new("data"), 200, 0).unwrap(), "");
    }
}
