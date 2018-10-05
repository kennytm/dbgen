use dbgen::{quote::Quote, Generator, RngSeed, Template};

use data_encoding::{DecodeError, DecodeKind, HEXLOWER_PERMISSIVE};
use failure::{Error, ResultExt};
use rand::{EntropyRng, Rng};
use structopt::StructOpt;

use std::fs::{create_dir_all, read_to_string, File};
use std::io::BufWriter;
use std::path::PathBuf;
use std::process::exit;

#[derive(StructOpt, Debug)]
struct Args {
    #[structopt(short = "d", long = "schema-name", help = "Schema name")]
    schema_name: String,

    #[structopt(short = "t", long = "table-name", help = "Table name")]
    table_name: String,

    #[structopt(
        short = "o",
        long = "out-dir",
        help = "Output directory",
        parse(from_os_str)
    )]
    out_dir: PathBuf,

    #[structopt(
        short = "k",
        long = "files-count",
        help = "Number of files to generate",
        default_value = "1"
    )]
    files_count: u32,

    #[structopt(
        short = "n",
        long = "inserts-count",
        help = "Number of INSERT statements per file"
    )]
    inserts_count: u32,

    #[structopt(
        short = "r",
        long = "rows-count",
        help = "Number of rows per INSERT statement",
        default_value = "1"
    )]
    rows_count: u32,

    #[structopt(
        short = "i",
        long = "template",
        help = "Generation template SQL",
        parse(from_os_str)
    )]
    template: PathBuf,

    #[structopt(
        short = "s",
        long = "seed",
        help = "Random number generator seed (should have 64 hex digits)",
        parse(try_from_str = "seed_from_str")
    )]
    seed: Option<RngSeed>,

    #[structopt(
        short = "q",
        long = "quote",
        help = "Identifier quote style",
        raw(possible_values = r#"&["double", "backquote", "brackets"]"#),
        default_value = "double",
        parse(from_str = "quote_from_str")
    )]
    quote: Quote,
}

fn quote_from_str(s: &str) -> Quote {
    match s {
        "double" => Quote::Double,
        "backquote" => Quote::Backquote,
        "brackets" => Quote::Brackets,
        _ => unreachable!(),
    }
}

fn seed_from_str(s: &str) -> Result<RngSeed, DecodeError> {
    let mut seed = RngSeed::default();

    if HEXLOWER_PERMISSIVE.decode_len(s.len())? != seed.len() {
        return Err(DecodeError {
            position: s.len(),
            kind: DecodeKind::Length,
        });
    }
    match HEXLOWER_PERMISSIVE.decode_mut(s.as_bytes(), &mut seed) {
        Ok(_) => Ok(seed),
        Err(e) => Err(e.error),
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{}\n", err);
        for (e, i) in err.iter_causes().zip(1..) {
            eprintln!("{:=^80}\n{}\n", format!(" ERROR CAUSE #{} ", i), e);
        }
        exit(1);
    }
}

fn run() -> Result<(), Error> {
    let args = Args::from_args();
    let input = read_to_string(&args.template).context("failed to read template")?;
    let template = Template::parse(&input)?;

    let generator = Generator::new(template, args.quote, &args.table_name);

    create_dir_all(&args.out_dir).context("failed to create output directory")?;
    let schema_path = args
        .out_dir
        .join(format!("{}.{}-schema.sql", args.schema_name, args.table_name));
    let schema_file = File::create(&schema_path)
        .with_context(|_| format!("failed to create schema file at {}", schema_path.display()))?;
    let schema_file = BufWriter::new(schema_file);
    generator
        .write_sql_schema(schema_file)
        .with_context(|_| format!("failed to write to schema file at {}", schema_path.display()))?;

    let seed = args.seed.unwrap_or_else(|| EntropyRng::new().gen::<RngSeed>());
    eprintln!("Using seed: {}", HEXLOWER_PERMISSIVE.encode(&seed));
    let mut compiled = generator.compile(seed)?;

    let num_digits = args.files_count.to_string().len();
    for file_index in 1..=args.files_count {
        let data_path = args.out_dir.join(format!(
            "{0}.{1}.{2:03$}.sql",
            args.schema_name, args.table_name, file_index, num_digits
        ));
        let data_file =
            File::create(&data_path).with_context(|_| format!("failed to data file at {}", data_path.display()))?;
        let mut data_file = BufWriter::new(data_file);
        for _ in 0..args.inserts_count {
            compiled
                .write_sql(&mut data_file, args.rows_count)
                .with_context(|_| format!("failed to write to data file at {}", data_path.display()))?;
        }
    }

    Ok(())
}
