//! CLI driver of `dbgen`.

use crate::{
    eval::{CompileContext, Row, State},
    format::{CsvFormat, Format, SqlFormat},
    parser::{QName, Template},
};

use chrono_tz::Tz;
use data_encoding::{DecodeError, DecodeKind, HEXLOWER_PERMISSIVE};
use failure::{Error, Fail, ResultExt};
use flate2::write::GzEncoder;
use muldiv::MulDiv;
use pbr::{MultiBar, Units};
use rand::{
    rngs::{EntropyRng, StdRng},
    Rng, RngCore, SeedableRng,
};
use rayon::{
    iter::{IntoParallelIterator, ParallelIterator},
    ThreadPoolBuilder,
};
use serde_derive::Deserialize;
use std::{
    fs::{create_dir_all, read_to_string, File},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread::{sleep, spawn},
    time::Duration,
};
use structopt::StructOpt;
use xz2::write::XzEncoder;

/// Arguments to the `dbgen` CLI program.
#[derive(StructOpt, Debug, Deserialize)]
#[serde(default)]
#[structopt(raw(long_version = "::FULL_VERSION"))]
pub struct Args {
    /// Keep the qualified name when writing the SQL statements.
    #[structopt(long = "qualified", help = "Keep the qualified name when writing the SQL statements")]
    pub qualified: bool,

    /// Override the table name.
    #[structopt(short = "t", long = "table-name", help = "Override the table name")]
    pub table_name: Option<String>,

    /// Output directory.
    #[structopt(short = "o", long = "out-dir", help = "Output directory", parse(from_os_str))]
    pub out_dir: PathBuf,

    /// Number of files to generate.
    #[structopt(
        short = "k",
        long = "files-count",
        help = "Number of files to generate",
        default_value = "1"
    )]
    pub files_count: u32,

    /// Number of INSERT statements per file.
    #[structopt(
        short = "n",
        long = "inserts-count",
        help = "Number of INSERT statements per file",
        default_value = "1"
    )]
    pub inserts_count: u32,

    /// Number of rows per INSERT statement.
    #[structopt(
        short = "r",
        long = "rows-count",
        help = "Number of rows per INSERT statement",
        default_value = "1"
    )]
    pub rows_count: u32,

    /// Number of INSERT statements in the last file.
    #[structopt(
        long = "last-file-inserts-count",
        help = "Number of INSERT statements in the last file"
    )]
    pub last_file_inserts_count: Option<u32>,

    /// Number of rows of the last INSERT statement of the last file.
    #[structopt(
        long = "last-insert-rows-count",
        help = "Number of rows of the last INSERT statement of the last file"
    )]
    pub last_insert_rows_count: Option<u32>,

    /// Do not escape backslashes when writing a string.
    #[structopt(long = "escape-backslash", help = "Escape backslashes when writing a string")]
    pub escape_backslash: bool,

    /// Generation template file.
    #[structopt(
        short = "i",
        long = "template",
        help = "Generation template file",
        parse(from_os_str)
    )]
    pub template: PathBuf,

    /// Random number generator seed.
    #[structopt(
        short = "s",
        long = "seed",
        help = "Random number generator seed (should have 64 hex digits)",
        parse(try_from_str = "seed_from_str")
    )]
    pub seed: Option<<StdRng as SeedableRng>::Seed>,

    /// Number of jobs to run in parallel, default to number of CPUs.
    #[structopt(
        short = "j",
        long = "jobs",
        help = "Number of jobs to run in parallel, default to number of CPUs",
        default_value = "0"
    )]
    pub jobs: usize,

    /// Random number generator engine
    #[structopt(
        long = "rng",
        help = "Random number generator engine",
        raw(possible_values = r#"&["chacha", "hc128", "isaac", "isaac64", "xorshift", "pcg32"]"#),
        default_value = "hc128"
    )]
    pub rng: RngName,

    /// Disable progress bar.
    #[structopt(short = "q", long = "quiet", help = "Disable progress bar")]
    pub quiet: bool,

    /// Timezone
    #[structopt(long = "time-zone", help = "Time zone used for timestamps", default_value = "UTC")]
    pub time_zone: Tz,

    /// Output format
    #[structopt(
        short = "f",
        long = "format",
        help = "Output format",
        raw(possible_values = r#"&["sql", "csv"]"#),
        default_value = "sql"
    )]
    pub format: FormatName,

    /// Output compression
    #[structopt(
        short = "c",
        long = "compress",
        help = "Compress data output",
        raw(possible_values = r#"&["gzip", "gz", "xz", "zstd", "zst"]"#)
    )]
    pub compression: Option<CompressionName>,

    /// Output compression level
    #[structopt(
        long = "compress-level",
        help = "Compression level (0-9 for gzip and xz, 1-21 for zstd)",
        default_value = "6"
    )]
    pub compress_level: u8,
}

/// The default implementation of the argument suitable for *testing*.
impl Default for Args {
    fn default() -> Self {
        Self {
            qualified: false,
            table_name: None,
            out_dir: PathBuf::default(),
            files_count: 1,
            inserts_count: 1,
            rows_count: 1,
            last_file_inserts_count: None,
            last_insert_rows_count: None,
            escape_backslash: false,
            template: PathBuf::default(),
            seed: None,
            jobs: 0,
            rng: RngName::Hc128,
            quiet: true,
            time_zone: Tz::UTC,
            format: FormatName::Sql,
            compression: None,
            compress_level: 6,
        }
    }
}

/// Parses a 64-digit hex string into an RNG seed.
pub(crate) fn seed_from_str(s: &str) -> Result<<StdRng as SeedableRng>::Seed, DecodeError> {
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

/// Extension trait for `Result` to annotate it with a file path.
trait PathResultExt {
    type Ok;
    fn with_path(self, path: &Path) -> Result<Self::Ok, Error>;
}

impl<T, E: Fail> PathResultExt for Result<T, E> {
    type Ok = T;
    fn with_path(self, path: &Path) -> Result<T, Error> {
        Ok(self.with_context(|_| format!("with file {}...", path.display()))?)
    }
}

/// Indicator whether all tables are written. Used by the progress bar thread to break the loop.
static WRITE_FINISHED: AtomicBool = AtomicBool::new(false);
/// Counter of number of rows being written.
static WRITE_PROGRESS: AtomicUsize = AtomicUsize::new(0);
/// Counter of number of bytes being written.
static WRITTEN_SIZE: AtomicUsize = AtomicUsize::new(0);

/// Runs the CLI program.
pub fn run(args: Args) -> Result<(), Error> {
    let input = read_to_string(&args.template).context("failed to read template")?;
    let template = Template::parse(&input)?;

    let pool = ThreadPoolBuilder::new()
        .num_threads(args.jobs)
        .build()
        .context("failed to configure thread pool")?;

    let table_name = match args.table_name {
        Some(n) => QName::parse(&n)?,
        None => template.name,
    };

    create_dir_all(&args.out_dir).context("failed to create output directory")?;

    let ctx = CompileContext {
        time_zone: args.time_zone,
    };

    let compress_level = args.compress_level;
    let env = Env {
        out_dir: args.out_dir,
        file_num_digits: args.files_count.to_string().len(),
        unique_name: table_name.unique_name(),
        row_gen: ctx.compile_row(template.exprs)?,
        qualified_name: if args.qualified {
            table_name.qualified_name()
        } else {
            table_name.table
        },
        rows_count: args.rows_count,
        escape_backslash: args.escape_backslash,
        format: args.format,
        compression: args.compression.map(|c| (c, compress_level)),
    };

    env.write_schema(&template.content)?;

    let meta_seed = args.seed.unwrap_or_else(|| EntropyRng::new().gen());
    let show_progress = !args.quiet;
    if show_progress {
        println!("Using seed: {}", HEXLOWER_PERMISSIVE.encode(&meta_seed));
    }
    let mut seeding_rng = StdRng::from_seed(meta_seed);

    let files_count = args.files_count;
    let variables_count = template.variables_count;
    let rows_per_file = u64::from(args.inserts_count) * u64::from(args.rows_count);
    let rng_name = args.rng;
    let inserts_count = args.inserts_count;
    let rows_count = args.rows_count;
    let last_file_inserts_count = args.last_file_inserts_count.unwrap_or(inserts_count);
    let last_insert_rows_count = args.last_insert_rows_count.unwrap_or(rows_count);

    let progress_bar_thread = spawn(move || {
        if show_progress {
            run_progress_thread(
                u64::from(files_count - 1) * rows_per_file
                    + u64::from(last_file_inserts_count - 1) * u64::from(rows_count)
                    + u64::from(last_insert_rows_count),
            );
        }
    });

    let iv = (0..files_count)
        .map(move |i| {
            let file_index = i + 1;
            (
                rng_name.create(&mut seeding_rng),
                FileInfo {
                    file_index,
                    inserts_count: if file_index == files_count {
                        last_file_inserts_count
                    } else {
                        inserts_count
                    },
                    last_insert_rows_count: if file_index == files_count {
                        last_insert_rows_count
                    } else {
                        rows_count
                    },
                },
                u64::from(i) * rows_per_file + 1,
            )
        })
        .collect::<Vec<_>>();
    let res = pool.install(move || {
        iv.into_par_iter().try_for_each(|(seed, file_info, row_num)| {
            let mut state = State::new(row_num, seed, variables_count, ctx.clone());
            env.write_data_file(&file_info, &mut state)
        })
    });

    WRITE_FINISHED.store(true, Ordering::Relaxed);
    progress_bar_thread.join().unwrap();

    res?;
    Ok(())
}

/// Names of random number generators supported by `dbgen`.
#[derive(Copy, Clone, Debug, Deserialize)]
pub enum RngName {
    /// ChaCha20
    ChaCha,
    /// HC-128
    Hc128,
    /// ISAAC
    Isaac,
    /// ISAAC-64
    Isaac64,
    /// Xorshift
    XorShift,
    /// PCG32
    Pcg32,
}

impl FromStr for RngName {
    type Err = Error;
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(match name {
            "chacha" => RngName::ChaCha,
            "hc128" => RngName::Hc128,
            "isaac" => RngName::Isaac,
            "isaac64" => RngName::Isaac64,
            "xorshift" => RngName::XorShift,
            "pcg32" => RngName::Pcg32,
            _ => failure::bail!("Unsupported RNG {}", name),
        })
    }
}

impl RngName {
    /// Creates an RNG engine given the name. The RNG engine instance will be seeded from `src`.
    fn create(self, src: &mut StdRng) -> Box<dyn RngCore + Send> {
        match self {
            RngName::ChaCha => Box::new(rand_chacha::ChaChaRng::from_seed(src.gen())),
            RngName::Hc128 => Box::new(rand_hc::Hc128Rng::from_seed(src.gen())),
            RngName::Isaac => Box::new(rand_isaac::IsaacRng::from_seed(src.gen())),
            RngName::Isaac64 => Box::new(rand_isaac::Isaac64Rng::from_seed(src.gen())),
            RngName::XorShift => Box::new(rand_xorshift::XorShiftRng::from_seed(src.gen())),
            RngName::Pcg32 => Box::new(rand_pcg::Pcg32::from_seed(src.gen())),
        }
    }
}

/// Names of output formats supported by `dbgen`.
#[derive(Copy, Clone, Debug, Deserialize)]
pub enum FormatName {
    /// SQL
    Sql,
    /// Csv
    Csv,
}

impl FromStr for FormatName {
    type Err = Error;
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(match name {
            "sql" => FormatName::Sql,
            "csv" => FormatName::Csv,
            _ => failure::bail!("Unsupported format {}", name),
        })
    }
}

impl FormatName {
    /// Obtains the file extension when using this format.
    fn extension(self) -> &'static str {
        match self {
            FormatName::Sql => "sql",
            FormatName::Csv => "csv",
        }
    }

    /// Creates a formatter writer given the name.
    fn create(self, escape_backslash: bool) -> Box<dyn Format> {
        match self {
            FormatName::Sql => Box::new(SqlFormat { escape_backslash }),
            FormatName::Csv => Box::new(CsvFormat { escape_backslash }),
        }
    }
}

/// Names of the compression output formats supported by `dbgen`.
#[derive(Copy, Clone, Debug, Deserialize)]
pub enum CompressionName {
    /// Compress as gzip format (`*.gz`).
    Gzip,
    /// Compress as xz format (`*.xz`).
    Xz,
    /// Compress as Zstandard format (`*.zst`).
    Zstd,
}

impl FromStr for CompressionName {
    type Err = Error;
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(match name {
            "gzip" | "gz" => CompressionName::Gzip,
            "xz" => CompressionName::Xz,
            "zstd" | "zst" => CompressionName::Zstd,
            _ => failure::bail!("Unsupported format {}", name),
        })
    }
}

impl CompressionName {
    /// Obtains the file extension when using this format.
    fn extension(self) -> &'static str {
        match self {
            CompressionName::Gzip => "gz",
            CompressionName::Xz => "xz",
            CompressionName::Zstd => "zst",
        }
    }

    /// Wraps a writer with a compression layer on top.
    fn wrap<'a, W: Write + 'a>(self, inner: W, level: u8) -> Box<dyn Write + 'a> {
        match self {
            CompressionName::Gzip => Box::new(GzEncoder::new(inner, flate2::Compression::new(level.into()))),
            CompressionName::Xz => Box::new(XzEncoder::new(inner, level.into())),
            CompressionName::Zstd => Box::new(
                zstd::Encoder::new(inner, level.into())
                    .expect("valid zstd encoder")
                    .auto_finish(),
            ),
        }
    }
}

/// Wrapping of a [`Write`] which counts how many bytes are written.
struct WriteCountWrapper<W: Write> {
    inner: W,
    count: usize,
    skip_write: bool,
}
impl<W: Write> WriteCountWrapper<W> {
    /// Creates a new [`WriteCountWrapper`] by wrapping another [`Write`].
    fn new(inner: W) -> Self {
        Self {
            inner,
            count: 0,
            skip_write: false,
        }
    }

    /// Commits the number of bytes written into the [`WRITTEN_SIZE`] global variable, then resets
    /// the byte count of this instance to zero.
    fn commit_bytes_written(&mut self) {
        WRITTEN_SIZE.fetch_add(self.count, Ordering::Relaxed);
        self.count = 0;
    }
}

impl<W: Write> Write for WriteCountWrapper<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes_written = if self.skip_write {
            buf.len()
        } else {
            self.inner.write(buf)?
        };
        self.count += bytes_written;
        Ok(bytes_written)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.skip_write {
            Ok(())
        } else {
            self.inner.flush()
        }
    }
}

/// The environmental data shared by all data writers.
struct Env {
    out_dir: PathBuf,
    file_num_digits: usize,
    row_gen: Row,
    unique_name: String,
    qualified_name: String,
    rows_count: u32,
    escape_backslash: bool,
    format: FormatName,
    compression: Option<(CompressionName, u8)>,
}

/// Information specific to a data file.
struct FileInfo {
    file_index: u32,
    inserts_count: u32,
    last_insert_rows_count: u32,
}

impl Env {
    /// Writes the `CREATE TABLE` schema file.
    fn write_schema(&self, content: &str) -> Result<(), Error> {
        let path = self.out_dir.join(format!("{}-schema.sql", self.unique_name));
        let mut file = BufWriter::new(File::create(&path).with_path(&path)?);
        writeln!(file, "CREATE TABLE {} {}", self.qualified_name, content).with_path(&path)
    }

    /// Writes a single data file.
    fn write_data_file(&self, info: &FileInfo, state: &mut State) -> Result<(), Error> {
        let mut path = self.out_dir.join(format!(
            "{0}.{1:02$}.{3}",
            self.unique_name,
            info.file_index,
            self.file_num_digits,
            self.format.extension(),
        ));

        let inner_writer = if let Some((compression, level)) = self.compression {
            let mut path_string = path.into_os_string();
            path_string.push(".");
            path_string.push(compression.extension());
            path = PathBuf::from(path_string);
            compression.wrap(File::create(&path).with_path(&path)?, level)
        } else {
            Box::new(File::create(&path).with_path(&path)?)
        };

        let mut file = WriteCountWrapper::new(BufWriter::new(inner_writer));
        file.skip_write = std::env::var("DBGEN_WRITE_TO_DEV_NULL")
            .map(|s| s == "1")
            .unwrap_or(false);
        let format = self.format.create(self.escape_backslash);

        for i in 0..info.inserts_count {
            format.write_header(&mut file, &self.qualified_name).with_path(&path)?;

            let rows_count = if i == info.inserts_count - 1 {
                info.last_insert_rows_count
            } else {
                self.rows_count
            };
            for row_index in 0..rows_count {
                if row_index != 0 {
                    format.write_row_separator(&mut file).with_path(&path)?;
                }

                let values = self.row_gen.eval(state).with_path(&path)?;
                for (col_index, value) in values.iter().enumerate() {
                    if col_index != 0 {
                        format.write_value_separator(&mut file).with_path(&path)?;
                    }
                    format.write_value(&mut file, value).with_path(&path)?;
                }
            }

            format.write_trailer(&mut file).with_path(&path)?;
            file.commit_bytes_written();
            WRITE_PROGRESS.fetch_add(rows_count as usize, Ordering::Relaxed);
        }
        Ok(())
    }
}

/// Runs the progress bar thread.
///
/// This function will loop and update the progress bar every 0.5 seconds, until [`WRITE_FINISHED`]
/// becomes `true`.
fn run_progress_thread(total_rows: u64) {
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::non_ascii_literal))]
    const TICK_FORMAT: &str = "🕐🕑🕒🕓🕔🕕🕖🕗🕘🕙🕚🕛";

    let mut mb = MultiBar::new();

    let mut pb = mb.create_bar(total_rows);

    let mut speed_bar = mb.create_bar(0);
    speed_bar.set_units(Units::Bytes);
    speed_bar.show_percent = false;
    speed_bar.show_time_left = false;
    speed_bar.show_tick = true;
    speed_bar.show_bar = false;
    speed_bar.tick_format(TICK_FORMAT);

    pb.message("Progress ");
    speed_bar.message("Size     ");

    let mb_thread = spawn(move || mb.listen());

    while !WRITE_FINISHED.load(Ordering::Relaxed) {
        sleep(Duration::from_millis(500));
        let rows_count = WRITE_PROGRESS.load(Ordering::Relaxed) as u64;
        pb.set(rows_count);

        let written_size = WRITTEN_SIZE.load(Ordering::Relaxed) as u64;
        if rows_count != 0 {
            speed_bar.total = written_size
                .mul_div_round(total_rows, rows_count)
                .unwrap_or_else(u64::max_value);
            speed_bar.set(written_size);
        }
    }

    pb.finish_println("Done!");
    speed_bar.finish();

    mb_thread.join().unwrap();
}
