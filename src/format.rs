//! Output formatter

use crate::value::{Bytes, Value};

use chrono::{Datelike, NaiveDateTime, Timelike};
use std::{
    io::{Error, Write},
    slice,
};

/// Wrapper of a writer which could serialize a value into a string.
pub trait Format {
    /// Writes a single value to the writer, formatted according to specific
    /// rules of this formatter.
    fn write_value(&mut self, value: &Value) -> Result<(), Error>;

    /// Writes the content of an INSERT statement before all rows.
    fn write_header(&mut self, qualified_table_name: &str) -> Result<(), Error>;

    /// Writes the separator between the every value.
    fn write_value_separator(&mut self) -> Result<(), Error>;

    /// Writes the separator between the every row.
    fn write_row_separator(&mut self) -> Result<(), Error>;

    /// Writes the content of an INSERT statement after all rows.
    fn write_trailer(&mut self) -> Result<(), Error>;
}

/// SQL formatter.
#[derive(Debug)]
pub struct SqlFormat<W: Write> {
    /// The underlying writer.
    pub writer: W,
    /// Whether to escapes backslashes when writing a string.
    pub escape_backslash: bool,
}

impl<W: Write> SqlFormat<W> {
    fn write_bytes(&mut self, bytes: &Bytes) -> Result<(), Error> {
        if bytes.is_binary() {
            self.writer.write_all(b"X'")?;
            for b in bytes.as_bytes() {
                write!(self.writer, "{:02X}", b)?;
            }
        } else {
            self.writer.write_all(b"'")?;
            for b in bytes.as_bytes() {
                self.writer.write_all(match *b {
                    b'\'' => b"''",
                    b'\\' if self.escape_backslash => b"\\\\",
                    b'\0' if self.escape_backslash => b"\\0",
                    _ => slice::from_ref(b),
                })?;
            }
        }
        self.writer.write_all(b"'")
    }

    fn write_timestamp(&mut self, timestamp: &NaiveDateTime) -> Result<(), Error> {
        // write!(output, "'{}'", timestamp.format(TIMESTAMP_FORMAT))?;
        write!(
            self.writer,
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
            write!(self.writer, ".{:06}", ns / 1000)?;
        }
        self.writer.write_all(b"'")
    }
}

impl<W: Write> Format for SqlFormat<W> {
    fn write_value(&mut self, value: &Value) -> Result<(), Error> {
        match value {
            Value::Null => self.writer.write_all(b"NULL"),
            Value::Number(number) => write!(self.writer, "{}", number),
            Value::Bytes(bytes) => self.write_bytes(bytes),
            Value::Timestamp(timestamp) => self.write_timestamp(timestamp),
            Value::Interval(interval) => write!(self.writer, "INTERVAL {} MICROSECOND", interval),
            Value::__NonExhaustive => Ok(()),
        }
    }

    fn write_header(&mut self, qualified_table_name: &str) -> Result<(), Error> {
        write!(self.writer, "INSERT INTO {} VALUES\n(", qualified_table_name)
    }

    fn write_value_separator(&mut self) -> Result<(), Error> {
        self.writer.write_all(b", ")
    }

    fn write_row_separator(&mut self) -> Result<(), Error> {
        self.writer.write_all(b"),\n(")
    }

    fn write_trailer(&mut self) -> Result<(), Error> {
        self.writer.write_all(b");\n")
    }
}
