use clap::Parser as _;
use dbgen::schemagen_cli::{print_script, Args};

fn main() {
    let args = Args::parse();
    print_script(&args);

    // if let Err(err) = run(args) {
    //     eprintln!("{}\n", err);
    //     for (e, i) in err.iter_causes().zip(1..) {
    //         eprintln!("{:=^80}\n{}\n", format!(" ERROR CAUSE #{} ", i), e);
    //     }
    //     exit(1);
    // }

    // let mut rng = thread_rng();
    // let table = gen_table(Dialect::MySQL, &mut rng, 1e9);

    // println!("{}", table.schema);
    // println!("rows: {}", table.rows_count);
}
