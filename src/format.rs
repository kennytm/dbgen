//! Output formatter

use crate::{bytes::ByteString, eval::Schema, value::Value};

use chrono::{DateTime, Datelike, TimeZone, Timelike};
use memchr::{memchr2_iter, memchr3_iter, memchr_iter};
use rand_regex::Encoding;
use std::{
    borrow::Cow,
    io::{Error, Write},
    slice,
};
use tzfile::ArcTz;

/// An shared format description of how to serialize values into strings.
pub trait Format {
    /// Writes a single value to the writer, formatted according to specific
    /// rules of this formatter.
    fn write_value(&self, writer: &mut dyn Write, value: &Value) -> Result<(), Error>;

    /// Writes the content at the beginning of each file.
    fn write_file_header(&self, writer: &mut dyn Write, schema: &Schema<'_>) -> Result<(), Error>;

    /// Writes the content of an INSERT statement before all rows.
    fn write_header(&self, writer: &mut dyn Write, schema: &Schema<'_>) -> Result<(), Error>;

    /// Writes the column name before a value.
    fn write_value_header(&self, writer: &mut dyn Write, column: &str) -> Result<(), Error>;

    /// Writes the separator between the every value.
    fn write_value_separator(&self, writer: &mut dyn Write) -> Result<(), Error>;

    /// Writes the separator between the every row.
    fn write_row_separator(&self, writer: &mut dyn Write) -> Result<(), Error>;

    /// Writes the content of an INSERT statement after all rows.
    fn write_trailer(&self, writer: &mut dyn Write) -> Result<(), Error>;
}

/// Common options for the formatters.
#[derive(Debug)]
pub struct Options {
    /// Whether to escapes backslashes when writing a string.
    pub escape_backslash: bool,
    /// Whether to include column names in the INSERT statements.
    pub headers: bool,
    /// The string to print for TRUE result.
    pub true_string: Cow<'static, str>,
    /// The string to print for FALSE result.
    pub false_string: Cow<'static, str>,
    /// The string to print for NULL result.
    pub null_string: Cow<'static, str>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            escape_backslash: false,
            headers: false,
            true_string: Cow::Borrowed("1"),
            false_string: Cow::Borrowed("0"),
            null_string: Cow::Borrowed("NULL"),
        }
    }
}

/// SQL formatter.
#[derive(Debug)]
pub struct SqlFormat<'a>(pub &'a Options);

/// CSV formatter.
#[derive(Debug)]
pub struct CsvFormat<'a>(pub &'a Options);

/// SQL formatter using the INSERT-SET form.
#[derive(Debug)]
pub struct SqlInsertSetFormat<'a>(pub &'a Options);

/// Writes a timestamp in ISO 8601 format.
fn write_timestamp(writer: &mut dyn Write, quote: &str, timestamp: &DateTime<ArcTz>) -> Result<(), Error> {
    write!(
        writer,
        "{}{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        quote,
        timestamp.year(),
        timestamp.month(),
        timestamp.day(),
        timestamp.hour(),
        timestamp.minute(),
        timestamp.second(),
    )?;
    let ns = timestamp.nanosecond();
    if ns != 0 {
        write!(writer, ".{:06}", ns / 1000)?;
    }
    writer.write_all(quote.as_bytes())
}

/// Writes a time interval in the standard SQL format.
fn write_interval(writer: &mut dyn Write, quote: &str, mut interval: i64) -> Result<(), Error> {
    writer.write_all(quote.as_bytes())?;
    if interval == i64::min_value() {
        return write!(writer, "-106751991 04:00:54.775808{quote}");
    } else if interval < 0 {
        interval = -interval;
        writer.write_all(b"-")?;
    }

    let seconds = interval / 1_000_000;
    let microseconds = interval % 1_000_000;

    let minutes = seconds / 60;
    let seconds = seconds % 60;

    let hours = minutes / 60;
    let minutes = minutes % 60;

    let days = hours / 24;
    let hours = hours % 24;

    if days > 0 {
        write!(writer, "{days} ")?;
    }
    write!(writer, "{hours:02}:{minutes:02}:{seconds:02}")?;
    if microseconds > 0 {
        write!(writer, ".{microseconds:06}")?;
    }

    writer.write_all(quote.as_bytes())
}

#[derive(Debug, Copy, Clone)]
#[allow(variant_size_differences)]
enum EscapeRule {
    Escape(&'static [u8]),
    Unescape(u8),
}

#[derive(Debug, Default)]
struct EscapeState {
    prev_end: usize,
    prev_byte: u8,
    cur_start: usize,
    cur_byte: u8,
    unescape_ready: bool,
}

impl EscapeState {
    fn set_cur_and_read_prev<'b>(&mut self, bytes: &'b [u8], cur: usize) -> &'b [u8] {
        self.cur_start = cur;
        self.cur_byte = bytes[cur];
        &bytes[self.prev_end..cur]
    }

    fn apply_rule<'b>(&mut self, rule: &'b EscapeRule) -> &'b [u8] {
        let ret = match rule {
            EscapeRule::Escape(replacement) => {
                self.unescape_ready = false;
                *replacement
            }
            EscapeRule::Unescape(b) => {
                if self.unescape_ready && *b == self.prev_byte && self.cur_start == self.prev_end {
                    self.unescape_ready = false;
                    &[]
                } else {
                    self.unescape_ready = true;
                    slice::from_ref(b)
                }
            }
        };
        self.prev_end = self.cur_start + 1;
        self.prev_byte = self.cur_byte;
        ret
    }
}

fn write_with_escape(writer: &mut dyn Write, bytes: &[u8], rules: &[(u8, EscapeRule)]) -> Result<(), Error> {
    let mut state = EscapeState::default();
    match *rules {
        [] => {}
        [(s1, r1)] => {
            for cur in memchr_iter(s1, bytes) {
                writer.write_all(state.set_cur_and_read_prev(bytes, cur))?;
                writer.write_all(state.apply_rule(&r1))?;
            }
        }
        [(s1, r1), (s2, r2)] => {
            for cur in memchr2_iter(s1, s2, bytes) {
                writer.write_all(state.set_cur_and_read_prev(bytes, cur))?;
                let rule = if state.cur_byte == s1 { r1 } else { r2 };
                writer.write_all(state.apply_rule(&rule))?;
            }
        }
        [(s1, r1), (s2, r2), (s3, r3)] => {
            for cur in memchr3_iter(s1, s2, s3, bytes) {
                writer.write_all(state.set_cur_and_read_prev(bytes, cur))?;
                let rule = match bytes[cur] {
                    b if b == s1 => r1,
                    b if b == s2 => r2,
                    _ => r3,
                };
                writer.write_all(state.apply_rule(&rule))?;
            }
        }
        _ => {
            for (cur, cur_byte) in bytes.iter().enumerate() {
                if let Some((_, rule)) = rules.iter().find(|(s, _)| s == cur_byte) {
                    writer.write_all(state.set_cur_and_read_prev(bytes, cur))?;
                    writer.write_all(state.apply_rule(rule))?;
                }
            }
        }
    }

    writer.write_all(&bytes[state.prev_end..])
}

impl Options {
    fn write_sql_bytes(&self, writer: &mut dyn Write, bytes: &ByteString) -> Result<(), Error> {
        if bytes.encoding() == Encoding::Binary {
            writer.write_all(b"X'")?;
            for b in bytes.as_bytes() {
                write!(writer, "{b:02X}")?;
            }
        } else {
            writer.write_all(b"'")?;
            write_with_escape(
                writer,
                bytes.as_bytes(),
                if self.escape_backslash {
                    &[
                        (b'\'', EscapeRule::Escape(b"''")),
                        (b'\\', EscapeRule::Escape(br"\\")),
                        (b'\0', EscapeRule::Escape(br"\0")),
                    ]
                } else {
                    &[(b'\'', EscapeRule::Escape(b"''"))]
                },
            )?;
        }
        writer.write_all(b"'")
    }

    /// Writes a value in SQL format.
    pub fn write_sql_value(&self, writer: &mut dyn Write, value: &Value) -> Result<(), Error> {
        match value {
            Value::Null => writer.write_all(self.null_string.as_bytes()),
            Value::Number(number) => number.write_io(writer, &self.true_string, &self.false_string),
            Value::Bytes(bytes) => self.write_sql_bytes(writer, bytes),
            Value::Timestamp(timestamp, tz) => write_timestamp(writer, "'", &tz.from_utc_datetime(timestamp)),
            Value::Interval(interval) => write_interval(writer, "'", *interval),
            Value::Array(array) => {
                writer.write_all(b"ARRAY[")?;
                for (i, item) in array.iter().enumerate() {
                    if i != 0 {
                        writer.write_all(b", ")?;
                    }
                    self.write_sql_value(writer, item)?;
                }
                writer.write_all(b"]")
            }
        }
    }
}

impl Format for SqlFormat<'_> {
    fn write_value(&self, writer: &mut dyn Write, value: &Value) -> Result<(), Error> {
        self.0.write_sql_value(writer, value)
    }

    fn write_file_header(&self, _: &mut dyn Write, _: &Schema<'_>) -> Result<(), Error> {
        Ok(())
    }

    fn write_header(&self, writer: &mut dyn Write, schema: &Schema<'_>) -> Result<(), Error> {
        write!(writer, "INSERT INTO {} ", schema.name)?;
        if self.0.headers {
            writer.write_all(b"(")?;
            for (i, col) in schema.column_names().enumerate() {
                if i != 0 {
                    writer.write_all(b", ")?;
                }
                writer.write_all(col.as_bytes())?;
            }
            writer.write_all(b") ")?;
        }
        writer.write_all(b"VALUES\n(")
    }

    fn write_value_header(&self, _: &mut dyn Write, _: &str) -> Result<(), Error> {
        Ok(())
    }

    fn write_value_separator(&self, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(b", ")
    }

    fn write_row_separator(&self, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(b"),\n(")
    }

    fn write_trailer(&self, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(b");\n")
    }
}

impl Format for SqlInsertSetFormat<'_> {
    fn write_value(&self, writer: &mut dyn Write, value: &Value) -> Result<(), Error> {
        self.0.write_sql_value(writer, value)
    }

    fn write_file_header(&self, _: &mut dyn Write, _: &Schema<'_>) -> Result<(), Error> {
        Ok(())
    }

    fn write_header(&self, writer: &mut dyn Write, schema: &Schema<'_>) -> Result<(), Error> {
        writeln!(writer, "INSERT INTO {} SET", schema.name)
    }

    fn write_value_header(&self, writer: &mut dyn Write, column: &str) -> Result<(), Error> {
        write!(writer, "{column} = ")
    }

    fn write_value_separator(&self, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(b",\n")
    }

    fn write_row_separator(&self, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(b";\n\n")
    }

    fn write_trailer(&self, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(b";\n\n")
    }
}

impl CsvFormat<'_> {
    fn write_bytes(&self, writer: &mut dyn Write, bytes: &ByteString) -> Result<(), Error> {
        writer.write_all(b"\"")?;
        write_with_escape(
            writer,
            bytes.as_bytes(),
            if self.0.escape_backslash {
                &[(b'"', EscapeRule::Escape(b"\"\"")), (b'\\', EscapeRule::Escape(br"\\"))]
            } else {
                &[(b'"', EscapeRule::Escape(b"\"\""))]
            },
        )?;
        writer.write_all(b"\"")
    }

    fn write_column_name(&self, writer: &mut dyn Write, name: &[u8]) -> Result<(), Error> {
        writer.write_all(b"\"")?;
        let (mut rules, name) = match name.first() {
            Some(b'"') => (Vec::new(), &name[1..(name.len() - 1)]),
            Some(b'`') => (
                vec![(b'`', EscapeRule::Unescape(b'`')), (b'"', EscapeRule::Escape(b"\"\""))],
                &name[1..(name.len() - 1)],
            ),
            Some(b'[') => (vec![(b'"', EscapeRule::Escape(b"\"\""))], &name[1..(name.len() - 1)]),
            _ => (Vec::new(), name),
        };
        if self.0.escape_backslash {
            rules.push((b'\\', EscapeRule::Escape(br"\\")));
        }
        write_with_escape(writer, name, &rules)?;
        writer.write_all(b"\"")
    }
}

impl Format for CsvFormat<'_> {
    fn write_value(&self, writer: &mut dyn Write, value: &Value) -> Result<(), Error> {
        match value {
            Value::Null => writer.write_all(self.0.null_string.as_bytes()),
            Value::Number(number) => number.write_io(writer, &self.0.true_string, &self.0.false_string),
            Value::Bytes(bytes) => self.write_bytes(writer, bytes),
            Value::Timestamp(timestamp, tz) => write_timestamp(writer, "", &tz.from_utc_datetime(timestamp)),
            Value::Interval(interval) => write_interval(writer, "", *interval),
            Value::Array(array) => {
                writer.write_all(b"{")?;
                for (i, item) in array.iter().enumerate() {
                    if i != 0 {
                        writer.write_all(b",")?;
                    }
                    self.write_value(writer, item)?;
                }
                writer.write_all(b"}")
            }
        }
    }

    fn write_file_header(&self, writer: &mut dyn Write, schema: &Schema<'_>) -> Result<(), Error> {
        if !self.0.headers {
            return Ok(());
        }
        for (i, col) in schema.column_names().enumerate() {
            if i != 0 {
                self.write_value_separator(writer)?;
            }
            self.write_column_name(writer, col.as_bytes())?;
        }
        self.write_row_separator(writer)
    }

    fn write_header(&self, _: &mut dyn Write, _: &Schema<'_>) -> Result<(), Error> {
        Ok(())
    }

    fn write_value_separator(&self, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(b",")
    }

    fn write_value_header(&self, _: &mut dyn Write, _: &str) -> Result<(), Error> {
        Ok(())
    }

    fn write_row_separator(&self, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(b"\n")
    }

    fn write_trailer(&self, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(b"\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_with_escape() {
        let test_cases: Vec<(&[u8], &[(u8, EscapeRule)], &[u8])> = vec![
            (b"10 o'clock", &[], b"10 o'clock"),
            (b"10 o'clock", &[(b'\'', EscapeRule::Escape(b"''"))], b"10 o''clock"),
            (
                b"<b>R&D</b>",
                &[
                    (b'<', EscapeRule::Escape(b"&lt;")),
                    (b'>', EscapeRule::Escape(b"&gt;")),
                    (b'&', EscapeRule::Escape(b"&amp;")),
                ],
                b"&lt;b&gt;R&amp;D&lt;/b&gt;",
            ),
            (
                br#"<b>"R&D"</b>"#,
                &[
                    (b'<', EscapeRule::Escape(b"&lt;")),
                    (b'>', EscapeRule::Escape(b"&gt;")),
                    (b'&', EscapeRule::Escape(b"&amp;")),
                    (b'"', EscapeRule::Escape(b"&quot;")),
                ],
                b"&lt;b&gt;&quot;R&amp;D&quot;&lt;/b&gt;",
            ),
            (
                b"`a'b``c`",
                &[(b'\'', EscapeRule::Escape(b"''")), (b'`', EscapeRule::Unescape(b'`'))],
                b"`a''b`c`",
            ),
        ];

        for (src, rules, expected) in test_cases {
            let mut writer = Vec::with_capacity(expected.len());
            write_with_escape(&mut writer, src, rules).unwrap();
            assert_eq!(&writer, expected);
        }
    }
}
