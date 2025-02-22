use clap::Parser as _;
use dbgen::{
    cli::{Args, run},
    span::Registry,
};

fn main() {
    let mut registry = Registry::default();
    if let Err(e) = run(Args::parse(), &mut registry) {
        eprintln!("{}", registry.describe(&e));
    }
}
