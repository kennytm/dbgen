//! Row generator.

use crate::{
    error::Error,
    eval::{Compiled, State},
    parser::Expr,
    value::Value,
};
use std::io::{self, Write};

/// Represents a row of compiled values.
#[derive(Debug)]
pub struct Row(Vec<Compiled>);

impl Row {
    /// Compiles a vector of parsed expressions into a row.
    pub fn compile(exprs: Vec<Expr>) -> Result<Self, Error> {
        Ok(Row(exprs
            .into_iter()
            .map(Compiled::compile)
            .collect::<Result<Vec<_>, Error>>()?))
    }

    /// Evaluates the row into a vector of values and updates the state.
    pub fn eval(&self, state: &mut State) -> Result<Vec<Value>, Error> {
        let result = self
            .0
            .iter()
            .map(|compiled| compiled.eval(state))
            .collect::<Result<_, _>>()?;
        state.row_num += 1;
        Ok(result)
    }

    /// Writes the values using SQL format.
    pub fn write_sql(values: Vec<Value>, mut output: impl Write) -> Result<(), io::Error> {
        for (i, value) in values.into_iter().enumerate() {
            output.write_all(if i == 0 { &b"("[..] } else { &b", "[..] })?;
            value.write_sql(&mut output)?;
        }
        output.write_all(b")")
    }
}
