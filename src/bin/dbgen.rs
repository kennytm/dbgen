use anyhow::Error;
use dbgen::cli::{run, Args};
use std::fmt;
use structopt::StructOpt;

fn main() -> Result<(), DisplayError> {
    Ok(run(Args::from_args())?)
}

struct DisplayError(Error);

impl From<Error> for DisplayError {
    fn from(e: Error) -> Self {
        Self(e)
    }
}

impl fmt::Debug for DisplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}\n", self.0)?;
        for (i, e) in self.0.chain().enumerate().skip(1) {
            writeln!(f, "{:=^80}\n{}\n", format!(" ERROR CAUSE #{} ", i), e)?;
        }
        Ok(())
    }
}
