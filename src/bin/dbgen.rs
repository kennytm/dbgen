use dbgen::{
    eval::{RngSeed, State},
    gen::Row,
    parser::{QName, Template},
};

use data_encoding::{DecodeError, DecodeKind, HEXLOWER_PERMISSIVE};
use failure::{Error, Fail, ResultExt};
use pbr::ProgressBar;
use rand::{EntropyRng, Rng};
use structopt::StructOpt;

use std::fs::{create_dir_all, read_to_string, File};
use std::io::{BufWriter, Stdout, Write};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::time::Duration;

#[derive(StructOpt, Debug)]
struct Args {
    #[structopt(
        long = "qualified",
        help = "Keep the qualified name when writing the SQL statements"
    )]
    qualified: bool,

    #[structopt(long = "table-name", help = "Override the table name")]
    table_name: Option<String>,

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

trait PathResultExt {
    type Ok;
    fn with_path(self, path: &Path) -> Result<Self::Ok, Error>;
}

impl<T, E: Fail> PathResultExt for Result<T, E> {
    type Ok = T;
    fn with_path(self, path: &Path) -> Result<Self::Ok, Error> {
        Ok(self.with_context(|_| format!("with file {}...", path.display()))?)
    }
}

fn run() -> Result<(), Error> {
    let args = Args::from_args();
    let input = read_to_string(&args.template).context("failed to read template")?;
    let template = Template::parse(&input)?;

    let table_name = match args.table_name {
        Some(n) => QName::parse(&n)?,
        None => template.name,
    };

    create_dir_all(&args.out_dir).context("failed to create output directory")?;

    let env = Env {
        out_dir: args.out_dir,
        file_num_digits: args.files_count.to_string().len(),
        unique_name: table_name.unique_name(),
        row_gen: Row::compile(template.exprs)?,
        qualified_name: if args.qualified {
            table_name.qualified_name()
        } else {
            table_name.table
        },
        inserts_count: args.inserts_count,
        rows_count: args.rows_count,
    };

    env.write_schema(&template.content)?;

    let seed = args.seed.unwrap_or_else(|| EntropyRng::new().gen::<RngSeed>());
    eprintln!("Using seed: {}", HEXLOWER_PERMISSIVE.encode(&seed));
    let mut state = State::from_seed(seed);

    let mut pb = ProgressBar::new((args.files_count as u64) * (args.inserts_count as u64) * (args.rows_count as u64));
    pb.set_max_refresh_rate(Some(Duration::from_millis(500)));

    for file_index in 1..=args.files_count {
        env.write_data_file(file_index, &mut pb, &mut state)?;
    }

    pb.finish_println("Done!");
    Ok(())
}

struct Env {
    out_dir: PathBuf,
    file_num_digits: usize,
    row_gen: Row,
    unique_name: String,
    qualified_name: String,
    inserts_count: u32,
    rows_count: u32,
}

impl Env {
    fn write_schema(&self, content: &str) -> Result<(), Error> {
        let path = self.out_dir.join(format!("{}-schema.sql", self.unique_name));
        let mut file = BufWriter::new(File::create(&path).with_path(&path)?);
        writeln!(file, "CREATE TABLE {} {}", self.qualified_name, content).with_path(&path)
    }

    fn write_data_file(&self, file_index: u32, pb: &mut ProgressBar<Stdout>, state: &mut State) -> Result<(), Error> {
        let path = self.out_dir.join(format!(
            "{0}.{1:02$}.sql",
            self.unique_name, file_index, self.file_num_digits
        ));
        let mut file = BufWriter::new(File::create(&path).with_path(&path)?);
        for _ in 0..self.inserts_count {
            writeln!(file, "INSERT INTO {} VALUES", self.qualified_name).with_path(&path)?;
            for row_index in 0..self.rows_count {
                self.row_gen.write_sql(state, &mut file).with_path(&path)?;
                file.write_all(if row_index == self.rows_count - 1 {
                    b";\n"
                } else {
                    b",\n"
                })
                .with_path(&path)?;
                pb.inc();
            }
        }
        Ok(())
    }
}
