//! Output formatter

use crate::value::{Bytes, Value};

use chrono::{DateTime, Datelike, TimeZone, Timelike};
use chrono_tz::Tz;
use std::{
    io::{Error, Write},
    slice,
};

/// Wrapper of a writer which could serialize a value into a string.
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

impl SqlFormat {
    fn write_bytes(&self, writer: &mut dyn Write, bytes: &Bytes) -> Result<(), Error> {
        if bytes.is_binary() {
            writer.write_all(b"X'")?;
            for b in bytes.as_bytes() {
                write!(writer, "{:02X}", b)?;
            }
        } else {
            writer.write_all(b"'")?;
            for b in bytes.as_bytes() {
                writer.write_all(match *b {
                    b'\'' => b"''",
                    b'\\' if self.escape_backslash => b"\\\\",
                    b'\0' if self.escape_backslash => b"\\0",
                    _ => slice::from_ref(b),
                })?;
            }
        }
        writer.write_all(b"'")
    }

    fn write_timestamp(&self, writer: &mut dyn Write, timestamp: &DateTime<Tz>) -> Result<(), Error> {
        // write!(output, "'{}'", timestamp.format(TIMESTAMP_FORMAT))?;
        write!(
            writer,
            "'{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
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
        writer.write_all(b"'")
    }
}

impl Format for SqlFormat {
    fn write_value(&self, writer: &mut dyn Write, value: &Value) -> Result<(), Error> {
        match value {
            Value::Null => writer.write_all(b"NULL"),
            Value::Number(number) => write!(writer, "{}", number),
            Value::Bytes(bytes) => self.write_bytes(writer, bytes),
            Value::Timestamp(timestamp, tz) => self.write_timestamp(writer, &tz.from_utc_datetime(&timestamp)),
            Value::Interval(interval) => write!(writer, "INTERVAL {} MICROSECOND", interval),
            Value::__NonExhaustive => Ok(()),
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
