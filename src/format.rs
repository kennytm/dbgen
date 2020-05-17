//! Output formatter

use crate::{bytes::ByteString, value::Value};

use chrono::{DateTime, Datelike, TimeZone, Timelike};
use chrono_tz::Tz;
use memchr::{memchr2_iter, memchr3_iter, memchr_iter};
use std::{
    io::{Error, Write},
    slice,
};

/// An shared format description of how to serialize values into strings.
pub trait Format {
    /// Writes a single value to the writer, formatted according to specific
    /// rules of this formatter.
    fn write_value(&self, writer: &mut dyn Write, value: &Value) -> Result<(), Error>;

    /// Writes the content of an INSERT statement before all rows.
    fn write_header(&self, writer: &mut dyn Write, qualified_table_name: &str) -> Result<(), Error>;

    /// Writes the separator between the every value.
    fn write_value_separator(&self, writer: &mut dyn Write) -> Result<(), Error>;

    /// Writes the separator between the every row.
    fn write_row_separator(&self, writer: &mut dyn Write) -> Result<(), Error>;

    /// Writes the content of an INSERT statement after all rows.
    fn write_trailer(&self, writer: &mut dyn Write) -> Result<(), Error>;
}

/// SQL formatter.
#[derive(Debug)]
pub struct SqlFormat {
    /// Whether to escapes backslashes when writing a string.
    pub escape_backslash: bool,
}

/// CSV formatter.
#[derive(Debug)]
pub struct CsvFormat {
    /// Whether to escapes backslashes when writing a string.
    pub escape_backslash: bool,
}

/// Writes a timestamp in ISO 8601 format.
fn write_timestamp(writer: &mut dyn Write, quote: &str, timestamp: &DateTime<Tz>) -> Result<(), Error> {
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
        return write!(writer, "-106751991 04:00:54.775808{}", quote);
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
        write!(writer, "{} ", days)?;
    }
    write!(writer, "{:02}:{:02}:{:02}", hours, minutes, seconds)?;
    if microseconds > 0 {
        write!(writer, ".{:06}", microseconds)?;
    }

    writer.write_all(quote.as_bytes())
}

fn write_with_escape(writer: &mut dyn Write, bytes: &[u8], rules: &[(u8, &[u8])]) -> Result<(), Error> {
    let mut prev = 0;
    match *rules {
        [] => {}
        [(s1, r1)] => {
            for cur in memchr_iter(s1, bytes) {
                writer.write_all(&bytes[prev..cur])?;
                writer.write_all(r1)?;
                prev = cur + 1;
            }
        }
        [(s1, r1), (s2, r2)] => {
            for cur in memchr2_iter(s1, s2, bytes) {
                writer.write_all(&bytes[prev..cur])?;
                writer.write_all(if bytes[cur] == s1 { r1 } else { r2 })?;
                prev = cur + 1;
            }
        }
        [(s1, r1), (s2, r2), (s3, r3)] => {
            for cur in memchr3_iter(s1, s2, s3, bytes) {
                writer.write_all(&bytes[prev..cur])?;
                writer.write_all(match bytes[cur] {
                    b if b == s1 => r1,
                    b if b == s2 => r2,
                    _ => r3,
                })?;
                prev = cur + 1;
            }
        }
        _ => {
            for cur in bytes {
                if let Some((_, r)) = rules.iter().find(|(s, _)| s == cur) {
                    writer.write_all(r)
                } else {
                    writer.write_all(slice::from_ref(cur))
                }?;
            }
            return Ok(());
        }
    }

    writer.write_all(&bytes[prev..])
}

impl SqlFormat {
    fn write_bytes(&self, writer: &mut dyn Write, bytes: &ByteString) -> Result<(), Error> {
        if bytes.is_binary() {
            writer.write_all(b"X'")?;
            for b in bytes.as_bytes() {
                write!(writer, "{:02X}", b)?;
            }
        } else {
            writer.write_all(b"'")?;
            write_with_escape(
                writer,
                bytes.as_bytes(),
                if self.escape_backslash {
                    &[(b'\'', b"''"), (b'\\', br"\\"), (b'\0', br"\0")]
                } else {
                    &[(b'\'', b"''")]
                },
            )?;
        }
        writer.write_all(b"'")
    }
}

impl Format for SqlFormat {
    fn write_value(&self, writer: &mut dyn Write, value: &Value) -> Result<(), Error> {
        match value {
            Value::Null => writer.write_all(b"NULL"),
            Value::Number(number) => write!(writer, "{}", number),
            Value::Bytes(bytes) => self.write_bytes(writer, bytes),
            Value::Timestamp(timestamp, tz) => write_timestamp(writer, "'", &tz.from_utc_datetime(timestamp)),
            Value::Interval(interval) => write_interval(writer, "'", *interval),
            Value::Array(array) => {
                writer.write_all(b"ARRAY[")?;
                for (i, item) in array.iter().enumerate() {
                    if i != 0 {
                        writer.write_all(b", ")?;
                    }
                    self.write_value(writer, item)?;
                }
                writer.write_all(b"]")
            }
        }
    }

    fn write_header(&self, writer: &mut dyn Write, qualified_table_name: &str) -> Result<(), Error> {
        write!(writer, "INSERT INTO {} VALUES\n(", qualified_table_name)
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

impl CsvFormat {
    fn write_bytes(&self, writer: &mut dyn Write, bytes: &ByteString) -> Result<(), Error> {
        writer.write_all(b"\"")?;
        write_with_escape(
            writer,
            bytes.as_bytes(),
            if self.escape_backslash {
                &[(b'"', b"\"\""), (b'\\', br"\\")]
            } else {
                &[(b'"', b"\"\"")]
            },
        )?;
        writer.write_all(b"\"")
    }
}

impl Format for CsvFormat {
    fn write_value(&self, writer: &mut dyn Write, value: &Value) -> Result<(), Error> {
        match value {
            Value::Null => writer.write_all(br"\N"),
            Value::Number(number) => write!(writer, "{}", number),
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

    fn write_header(&self, _: &mut dyn Write, _: &str) -> Result<(), Error> {
        Ok(())
    }

    fn write_value_separator(&self, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(b",")
    }

    fn write_row_separator(&self, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(b"\n")
    }

    fn write_trailer(&self, writer: &mut dyn Write) -> Result<(), Error> {
        writer.write_all(b"\n")
    }
}
