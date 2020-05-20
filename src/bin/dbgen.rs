use dbgen::{
    cli::{run, Args},
    span::Registry,
};
use structopt::StructOpt;

fn main() {
    let mut registry = Registry::default();
    if let Err(e) = run(Args::from_args(), &mut registry) {
        eprintln!("{}", registry.describe(e));
    }
}
