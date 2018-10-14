use crate::{
    error::Error,
    eval::{Compiled, State},
    parser::Expr,
    value::Value,
};
use std::io::{self, Write};

#[derive(Debug)]
pub struct Row(Vec<Compiled>);

impl Row {
    pub fn compile(exprs: Vec<Expr>) -> Result<Self, Error> {
        Ok(Row(exprs
            .into_iter()
            .map(Compiled::compile)
            .collect::<Result<Vec<_>, Error>>()?))
    }

    pub fn eval(&self, state: &mut State) -> Result<Vec<Value>, Error> {
        let result = self
            .0
            .iter()
            .map(|compiled| compiled.eval(state))
            .collect::<Result<_, _>>()?;
        state.row_num += 1;
        Ok(result)
    }

    pub fn write_sql(values: Vec<Value>, mut output: impl Write) -> Result<(), io::Error> {
        for (i, value) in values.into_iter().enumerate() {
            output.write_all(if i == 0 { &b"("[..] } else { &b", "[..] })?;
            value.write_sql(&mut output)?;
        }
        output.write_all(b")")
    }
}
