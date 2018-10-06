use crate::{
    error::{Error, ErrorKind},
    eval::{Compiled, State},
    parser::Expr,
};
use failure::ResultExt;
use std::io::Write;

pub struct Row(Vec<Compiled>);

impl Row {
    pub fn compile(exprs: Vec<Expr>) -> Result<Self, Error> {
        Ok(Row(exprs
            .into_iter()
            .map(Compiled::compile)
            .collect::<Result<Vec<_>, Error>>()?))
    }

    pub fn write_sql(&self, state: &mut State, mut output: impl Write) -> Result<(), Error> {
        for (i, compiled) in self.0.iter().enumerate() {
            output
                .write_all(if i == 0 { &b"("[..] } else { &b", "[..] })
                .context(ErrorKind::WriteSqlData)?;
            compiled
                .eval(state)?
                .write_sql(&mut output)
                .context(ErrorKind::WriteSqlData)?;
        }
        output.write_all(b")").context(ErrorKind::WriteSqlData)?;
        state.row_num += 1;
        Ok(())
    }
}
