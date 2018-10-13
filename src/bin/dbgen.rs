use dbgen::cli::{run, Args};
use std::process::exit;
use structopt::StructOpt;

fn main() {
    let args = Args::from_args();
    if let Err(err) = run(args) {
        eprintln!("{}\n", err);
        for (e, i) in err.iter_causes().zip(1..) {
            eprintln!("{:=^80}\n{}\n", format!(" ERROR CAUSE #{} ", i), e);
        }
        exit(1);
    }
}
