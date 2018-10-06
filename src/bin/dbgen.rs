// TODO remove all #[cfg_attr(feature = "cargo-clippy")] once tool_lints is stabilized.
#![cfg_attr(feature = "cargo-clippy", feature(tool_lints))]
#![cfg_attr(feature = "cargo-clippy", warn(clippy::pedantic))]

// TODO remove these `extern crate` once RLS understands these are not needed.
extern crate data_encoding;
extern crate failure;
extern crate pbr;
extern crate rand;
extern crate rayon;
extern crate structopt;

use dbgen::{
    eval::State,
    gen::Row,
    parser::{QName, Template},
};

use data_encoding::{DecodeError, DecodeKind, HEXLOWER_PERMISSIVE};
use failure::{Error, Fail, ResultExt};
use pbr::ProgressBar;
use rand::{EntropyRng, Rng, SeedableRng, StdRng};
use rayon::{
    iter::{IntoParallelIterator, ParallelIterator},
    ThreadPoolBuilder,
};
use std::{
    fs::{create_dir_all, read_to_string, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::exit,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread::{sleep, spawn},
    time::Duration,
};
use structopt::StructOpt;

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
    seed: Option<<StdRng as SeedableRng>::Seed>,

    #[structopt(
        short = "j",
        long = "jobs",
        help = "Number of jobs to run in parallel, default to number of CPUs",
        default_value = "0"
    )]
    jobs: usize,
}

fn seed_from_str(s: &str) -> Result<<StdRng as SeedableRng>::Seed, DecodeError> {
    let mut seed = <StdRng as SeedableRng>::Seed::default();

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

#[cfg_attr(feature = "cargo-clippy", allow(clippy::use_self))] // issue rust-clippy#1993
impl<T, E: Fail> PathResultExt for Result<T, E> {
    type Ok = T;
    fn with_path(self, path: &Path) -> Result<Self::Ok, Error> {
        Ok(self.with_context(|_| format!("with file {}...", path.display()))?)
    }
}

static WRITE_FINISHED: AtomicBool = AtomicBool::new(false);
static WRITE_PROGRESS: AtomicUsize = AtomicUsize::new(0);

fn run() -> Result<(), Error> {
    let args = Args::from_args();
    let input = read_to_string(&args.template).context("failed to read template")?;
    let template = Template::parse(&input)?;

    ThreadPoolBuilder::new()
        .num_threads(args.jobs)
        .build_global()
        .context("failed to configure thread pool")?;

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

    let meta_seed = args.seed.unwrap_or_else(|| EntropyRng::new().gen());
    eprintln!("Using seed: {}", HEXLOWER_PERMISSIVE.encode(&meta_seed));
    let mut seeding_rng = StdRng::from_seed(meta_seed);

    let files_count = args.files_count;
    let variables_count = template.variables_count;
    let rows_per_file = u64::from(args.inserts_count) * u64::from(args.rows_count);

    let progress_bar_thread = spawn(move || {
        let total_rows = u64::from(files_count) * rows_per_file;
        let mut pb = ProgressBar::new(total_rows);
        while !WRITE_FINISHED.load(Ordering::Relaxed) {
            sleep(Duration::from_millis(500));
            pb.set(WRITE_PROGRESS.load(Ordering::Relaxed) as u64);
        }
        pb.finish_println("Done!");
    });

    let iv = (0..files_count)
        .map(|i| (seeding_rng.gen(), i + 1, u64::from(i) * rows_per_file + 1))
        .collect::<Vec<_>>();
    let res = iv.into_par_iter().try_for_each(|(seed, file_index, row_num)| {
        let mut state = State::new(row_num, seed, variables_count);
        env.write_data_file(file_index, &mut state)
    });

    WRITE_FINISHED.store(true, Ordering::Relaxed);
    progress_bar_thread.join().unwrap();

    res?;
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

    fn write_data_file(&self, file_index: u32, state: &mut State) -> Result<(), Error> {
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
            }
            WRITE_PROGRESS.fetch_add(self.rows_count as usize, Ordering::Relaxed);
        }
        Ok(())
    }
}
