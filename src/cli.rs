//! CLI driver of `dbgen`.

use crate::{
    error::Error,
    eval::{CompileContext, Schema, State, Table},
    format::{CsvFormat, Format, Options, SqlFormat, SqlInsertSetFormat},
    lexctr::LexCtr,
    parser::{QName, Template},
    span::{Registry, ResultExt, SpanExt, S},
    value::{Value, TIMESTAMP_FORMAT},
    writer::{self, Writer},
};

use chrono::{NaiveDateTime, ParseResult, Utc};
use data_encoding::{DecodeError, DecodeKind, HEXLOWER_PERMISSIVE};
use flate2::write::GzEncoder;
use muldiv::MulDiv;
use pbr::{MultiBar, Units};
use rand::{
    distributions::{Distribution, Standard},
    rngs::{mock::StepRng, OsRng},
    Rng, RngCore, SeedableRng,
};
use rayon::{
    iter::{IntoParallelIterator, ParallelIterator},
    ThreadPoolBuilder,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    borrow::Cow,
    collections::HashMap,
    convert::TryInto,
    fmt,
    fs::{create_dir_all, read_to_string, File},
    io::{self, sink, stdin, BufWriter, Read, Write},
    mem,
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    thread::{sleep, spawn},
    time::Duration,
};
use structopt::{
    clap::AppSettings::{NextLineHelp, UnifiedHelpMessage},
    StructOpt,
};
use xz2::write::XzEncoder;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
struct RowArgs {
    /// Number of files to generate (*k*).
    files_count: u32,
    /// Number of INSERT statements per normal file (*n*).
    inserts_count: u32,
    /// Number of INSERT statements in the last file.
    last_file_inserts_count: u32,
    /// Number of rows per INSERT statement (*r*).
    rows_count: u32,
    /// Number of rows in the final INSERT statement in a normal file.
    final_insert_rows_count: u32,
    /// Number of rows in the final INSERT statement in the last file.
    last_file_final_insert_rows_count: u32,

    /// Number of rows per normal file (*R*).
    ///
    /// Must be same as `(n - 1) * r + final_insert_rows_count`.
    rows_per_file: u64,

    /// Total number of rows to generate (*N*).
    ///
    /// Must be same as `(k - 1) * R + (last_file_inserts_count - 1) * r + last_file_final_insert_rows_count`.
    total_count: u64,
}

/// Arguments to the `dbgen` CLI program.
#[derive(StructOpt, Debug, Serialize, Deserialize)]
#[serde(default)]
#[structopt(long_version(crate::FULL_VERSION), settings(&[NextLineHelp, UnifiedHelpMessage]))]
#[allow(clippy::struct_excessive_bools)]
pub struct Args {
    /// Keep the qualified name when writing the SQL statements.
    #[structopt(long)]
    #[serde(skip_serializing_if = "is_false")]
    pub qualified: bool,

    /// Override the table name.
    #[structopt(short, long, conflicts_with("schema-name"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_name: Option<String>,

    /// Override the schema name.
    #[structopt(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_name: Option<String>,

    /// Output directory.
    #[structopt(short, long, parse(from_os_str))]
    pub out_dir: PathBuf,

    /// Total number of file generator threads.
    #[structopt(short = "k", long, default_value = "1")]
    #[serde(skip_serializing_if = "is_one")]
    pub files_count: u32,

    /// Number of INSERT statements per file generator threads.
    #[structopt(short = "n", long, default_value = "1")]
    #[serde(skip_serializing_if = "is_one")]
    pub inserts_count: u32,

    /// Number of rows per INSERT statement.
    #[structopt(short, long, default_value = "1")]
    pub rows_count: u32,

    /// Number of INSERT statements in the last file generator thread.
    #[structopt(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_file_inserts_count: Option<u32>,

    /// Number of rows of the last INSERT statement of the last file generator thread.
    #[structopt(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_insert_rows_count: Option<u32>,

    /// Total number of rows of the main table.
    #[structopt(short = "N", long, parse(try_from_str = parse_row_count), conflicts_with_all(&["files-count", "last-file-inserts-count", "last-insert-rows-count"]))]
    pub total_count: Option<u64>,

    /// Number of rows per file generator thread.
    #[structopt(short = "R", long, parse(try_from_str = parse_row_count), conflicts_with_all(&["inserts-count"]))]
    pub rows_per_file: Option<u64>,

    /// Target pre-compressed size of each file.
    #[structopt(short = "z", long, parse(try_from_str = parse_size::parse_size))]
    pub size: Option<u64>,

    /// Escape backslashes when writing a string.
    #[structopt(long)]
    #[serde(skip_serializing_if = "is_false")]
    pub escape_backslash: bool,

    /// Generation template file.
    #[structopt(
        short = "i",
        long,
        parse(from_os_str),
        conflicts_with("template-string"),
        required_unless("template-string")
    )]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<PathBuf>,

    /// Inline generation template string.
    #[structopt(short = "e", long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_string: Option<String>,

    /// Random number generator seed (should have 64 hex digits).
    #[structopt(short, long)]
    pub seed: Option<Seed>,

    /// Number of jobs to run in parallel, default to number of CPUs.
    #[structopt(short, long, default_value = "0")]
    #[serde(skip_serializing_if = "is_zero")]
    pub jobs: usize,

    /// Random number generator engine.
    #[structopt(long, possible_values(&["chacha12", "chacha20", "hc128", "isaac", "isaac64", "xorshift", "pcg32", "step"]), default_value = "hc128")]
    #[serde(skip_serializing_if = "is_hc128")]
    pub rng: RngName,

    /// Disable progress bar.
    #[structopt(short, long)]
    #[serde(skip_serializing_if = "is_false")]
    pub quiet: bool,

    /// Time zone used for timestamps.
    #[structopt(long, default_value = "UTC")]
    #[serde(skip_serializing_if = "is_utc")]
    pub time_zone: String,

    /// Directory containing the tz database.
    #[structopt(long, parse(from_os_str), default_value = "/usr/share/zoneinfo")]
    #[serde(skip_serializing_if = "is_default_zoneinfo")]
    pub zoneinfo: PathBuf,

    /// Override the current timestamp (always in UTC), in the format "YYYY-mm-dd HH:MM:SS.fff".
    #[structopt(long, parse(try_from_str = now_from_str))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub now: Option<NaiveDateTime>,

    /// Output format.
    #[structopt(short, long, possible_values(&["sql", "csv", "sql-insert-set"]), default_value = "sql")]
    #[serde(skip_serializing_if = "is_sql")]
    pub format: FormatName,

    /// The keyword to print for a boolean TRUE value.
    #[structopt(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_true: Option<String>,

    /// The keyword to print for a boolean FALSE value.
    #[structopt(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_false: Option<String>,

    /// The keyword to print for a NULL value.
    #[structopt(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_null: Option<String>,

    /// Include column names or headers in the output.
    #[structopt(long)]
    #[serde(skip_serializing_if = "is_false")]
    pub headers: bool,

    /// Compress data output.
    #[structopt(short, long, possible_values(&["gzip", "gz", "xz", "zstd", "zst"]))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<CompressionName>,

    /// Compression level (0-9 for gzip and xz, 1-21 for zstd).
    #[structopt(long, default_value = "6")]
    #[serde(skip_serializing_if = "is_six")]
    pub compress_level: u8,

    /// Components to write.
    #[structopt(long, use_delimiter(true), possible_values(&["schema", "table", "data"]), default_value = "table,data", conflicts_with_all(&["no-schemas", "no-data"]))]
    #[serde(skip_serializing_if = "is_default_components")]
    pub components: Vec<ComponentName>,

    /// Do not generate schema files (the CREATE TABLE *.sql files).
    #[structopt(long, hidden(true))]
    #[serde(skip)]
    pub no_schemas: bool,

    /// Do not generate data files (only useful for benchmarking and fuzzing).
    #[structopt(long, hidden(true))]
    #[serde(skip)]
    pub no_data: bool,

    /// Initializes the template with these global expressions.
    #[structopt(long, short = "D")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub initialize: Vec<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            qualified: false,
            table_name: None,
            schema_name: None,
            out_dir: PathBuf::default(),
            files_count: 1,
            inserts_count: 1,
            rows_count: 1,
            last_file_inserts_count: None,
            last_insert_rows_count: None,
            total_count: None,
            rows_per_file: None,
            size: None,
            escape_backslash: false,
            template: None,
            template_string: None,
            seed: None,
            jobs: 0,
            rng: RngName::Hc128,
            quiet: false,
            time_zone: "UTC".to_owned(),
            zoneinfo: PathBuf::from("/usr/share/zoneinfo"),
            now: None,
            format: FormatName::Sql,
            format_true: None,
            format_false: None,
            format_null: None,
            headers: false,
            compression: None,
            compress_level: 6,
            components: vec![ComponentName::Table, ComponentName::Data],
            no_schemas: false,
            no_data: false,
            initialize: Vec::new(),
        }
    }
}

fn div_rem_plus_one(n: u64, d: u64) -> (u64, u64) {
    let (div, rem) = (n / d, n % d);
    if rem == 0 {
        (div, d)
    } else {
        (div + 1, rem)
    }
}

// the arguments of these serde helper must be references.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(b: &bool) -> bool {
    !*b
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_one(u: &u32) -> bool {
    *u == 1
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(u: &usize) -> bool {
    *u == 0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_six(u: &u8) -> bool {
    *u == 6
}

fn is_utc(tz: &str) -> bool {
    tz == "UTC"
}

fn is_default_zoneinfo(path: &Path) -> bool {
    path == Path::new("/usr/share/zoneinfo")
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_hc128(rng: &RngName) -> bool {
    *rng == RngName::Hc128
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_sql(format: &FormatName) -> bool {
    *format == FormatName::Sql
}

fn is_default_components(components: &[ComponentName]) -> bool {
    ComponentName::union_all(components.iter().copied()) == ComponentName::Table as u8 | ComponentName::Data as u8
}

fn parse_row_count(input: &str) -> Result<u64, parse_size::Error> {
    use parse_size::{ByteSuffix, Config};
    Config::new().with_byte_suffix(ByteSuffix::Deny).parse_size(input)
}

impl Args {
    /// Computes the row-related arguments.
    fn row_args(&self) -> RowArgs {
        let mut res = RowArgs {
            rows_count: self.rows_count,
            ..RowArgs::default()
        };

        // compute the rows per file.
        let rows_count = u64::from(self.rows_count);
        if let Some(rows_per_file) = self.rows_per_file {
            let (inserts_count, final_insert_rows_count) = div_rem_plus_one(rows_per_file, rows_count);
            res.inserts_count = inserts_count.try_into().expect("--rows-per-file is too large");
            res.final_insert_rows_count = final_insert_rows_count.try_into().unwrap();
            res.rows_per_file = rows_per_file;
        } else {
            res.inserts_count = self.inserts_count;
            res.final_insert_rows_count = self.rows_count;
            res.rows_per_file = u64::from(self.inserts_count) * rows_count;
        }

        // compute the total number of rows.
        if let Some(total_rows_count) = self.total_count {
            let (files_count, excess_rows_count) = div_rem_plus_one(total_rows_count, res.rows_per_file);
            res.files_count = files_count.try_into().expect("--total-count is too large");
            if excess_rows_count == res.rows_per_file {
                res.last_file_inserts_count = res.inserts_count;
                res.last_file_final_insert_rows_count = res.final_insert_rows_count;
            } else {
                let (inserts_count, final_insert_rows_count) = div_rem_plus_one(excess_rows_count, rows_count);
                res.last_file_inserts_count = inserts_count.try_into().expect("--rows-per-file is too large");
                res.last_file_final_insert_rows_count = final_insert_rows_count.try_into().unwrap();
            }
            res.total_count = total_rows_count;
        } else {
            res.files_count = self.files_count;
            res.last_file_inserts_count = self.last_file_inserts_count.unwrap_or(res.inserts_count);
            res.last_file_final_insert_rows_count = self.last_insert_rows_count.unwrap_or(res.final_insert_rows_count);
            res.total_count = u64::from(res.files_count - 1) * res.rows_per_file
                + u64::from(res.last_file_inserts_count - 1) * rows_count
                + u64::from(res.last_file_final_insert_rows_count);
        }

        res
    }
}

fn now_from_str(s: &str) -> ParseResult<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, TIMESTAMP_FORMAT)
}

/// Extension trait for `Result` to annotate it with a file path.
trait PathResultExt {
    type Ok;
    fn with_path(self, action: &'static str, path: &Path) -> Result<Self::Ok, S<Error>>;
    fn with_path_fn(self, action: &'static str, path_fn: impl FnOnce() -> PathBuf) -> Result<Self::Ok, S<Error>>;
}

impl<T> PathResultExt for io::Result<T> {
    type Ok = T;

    fn with_path(self, action: &'static str, path: &Path) -> Result<T, S<Error>> {
        self.with_path_fn(action, || path.to_owned())
    }

    fn with_path_fn(self, action: &'static str, path_fn: impl FnOnce() -> PathBuf) -> Result<T, S<Error>> {
        self.map_err(|source| {
            Error::Io {
                action,
                path: path_fn(),
                source,
            }
            .no_span()
        })
    }
}

/// Indicator whether all tables are written. Used by the progress bar thread to break the loop.
static WRITE_FINISHED: AtomicBool = AtomicBool::new(false);
/// Counter of number of rows being written.
static WRITE_PROGRESS: AtomicU64 = AtomicU64::new(0);
/// Counter of number of bytes being written.
static WRITTEN_SIZE: AtomicU64 = AtomicU64::new(0);

/// Reads the template file
fn read_template_file(path: &Path) -> Result<String, S<Error>> {
    if path == Path::new("-") {
        let mut buf = String::new();
        stdin().read_to_string(&mut buf).map(move |_| buf)
    } else {
        read_to_string(path)
    }
    .with_path("read template", path)
}

/// Runs the CLI program.
pub fn run(args: Args, span_registry: &mut Registry) -> Result<(), S<Error>> {
    let row_args = args.row_args();
    let input = match (args.template_string, &args.template) {
        (Some(input), _) => input,
        (None, Some(template)) => read_template_file(template)?,
        _ => {
            return Err(Error::UnsupportedCliParameter {
                kind: "template",
                value: "".to_owned(),
            }
            .no_span())
        }
    };
    let mut template = Template::parse(&input, &args.initialize, args.schema_name.as_deref(), span_registry)?;

    let pool = ThreadPoolBuilder::new().num_threads(args.jobs).build().no_span_err()?;

    if let Some(override_table_name) = &args.table_name {
        if template.tables.len() != 1 {
            return Err(Error::CannotUseTableNameForMultipleTables.no_span());
        }
        template.tables[0].name = QName::parse(override_table_name).no_span_err()?;
    }

    let mut ctx = CompileContext::new(template.variables_count);
    ctx.zoneinfo = args.zoneinfo;
    ctx.time_zone = ctx.parse_time_zone(&args.time_zone).no_span_err()?;
    ctx.current_timestamp = args.now.unwrap_or_else(|| Utc::now().naive_utc());
    let tables = template
        .tables
        .into_iter()
        .map(|t| ctx.compile_table(t))
        .collect::<Result<_, _>>()?;

    create_dir_all(&args.out_dir).with_path("create output directory", &args.out_dir)?;

    let compress_level = args.compress_level;
    let mut components_mask = ComponentName::union_all(args.components);
    if args.no_data {
        ComponentName::Data.remove_from(&mut components_mask);
    }
    if args.no_schemas {
        ComponentName::Schema.remove_from(&mut components_mask);
        ComponentName::Table.remove_from(&mut components_mask);
    }
    let format = args.format;
    let env = Env {
        out_dir: args.out_dir,
        file_num_digits: args.files_count.to_string().len(),
        tables,
        qualified: args.qualified,
        rows_count: args.rows_count,
        format,
        format_options: Options {
            escape_backslash: args.escape_backslash,
            headers: args.headers,
            true_string: args
                .format_true
                .map_or_else(|| format.default_true_string(), Cow::Owned),
            false_string: args
                .format_false
                .map_or_else(|| format.default_false_string(), Cow::Owned),
            null_string: args
                .format_null
                .map_or_else(|| format.default_null_string(), Cow::Owned),
        },
        compression: args.compression.map(|c| (c, compress_level)),
        components_mask,
        file_size: args.size,
    };

    if ComponentName::Schema.is_in(env.components_mask) {
        env.write_schema_schema()?;
    }
    if ComponentName::Table.is_in(env.components_mask) {
        env.write_table_schema()?;
    }

    let meta_seed = args.seed.unwrap_or_else(|| OsRng.gen());
    let show_progress = !args.quiet;
    if show_progress {
        println!("Using seed: {}", meta_seed);
    }
    let mut seeding_rng = meta_seed.make_rng();

    let rng_name = args.rng;

    // Evaluate the global expressions if necessary.
    if !template.global_exprs.is_empty() {
        let row_gen = ctx.compile_row(template.global_exprs)?;
        let mut state = State::new(0, rng_name.create(&mut seeding_rng), ctx);
        row_gen.eval(&mut state)?;
        ctx = state.into_compile_context();
    }

    WRITE_FINISHED.store(false, Ordering::Relaxed);
    WRITE_PROGRESS.store(0, Ordering::Relaxed);
    WRITTEN_SIZE.store(0, Ordering::Relaxed);

    let progress_bar_thread = spawn(move || {
        if show_progress {
            run_progress_thread(row_args.total_count);
        }
    });

    let iv = (0..row_args.files_count)
        .map(move |i| {
            let file_index = i + 1;
            (
                rng_name.create(&mut seeding_rng),
                FileInfo {
                    file_index,
                    inserts_count: if file_index == row_args.files_count {
                        row_args.last_file_inserts_count
                    } else {
                        row_args.inserts_count
                    },
                    last_insert_rows_count: if file_index == row_args.files_count {
                        row_args.last_file_final_insert_rows_count
                    } else {
                        row_args.final_insert_rows_count
                    },
                },
                u64::from(i) * row_args.rows_per_file + 1,
            )
        })
        .collect::<Vec<_>>();
    let res = pool.install(move || {
        iv.into_par_iter().try_for_each(|(seed, file_info, row_num)| {
            let mut state = State::new(row_num, seed, ctx.clone());
            env.write_data_file(&file_info, &mut state)
        })
    });

    WRITE_FINISHED.store(true, Ordering::Relaxed);
    progress_bar_thread.join().unwrap();

    res?;
    Ok(())
}

/// Random number generator (RNG) seed.
///
/// This is represented as a 64-digit hex string and is supposed to seed the
/// HC-128 RNG only.
#[derive(Copy, Clone, Debug, Default)]
pub struct Seed(<rand_hc::Hc128Rng as SeedableRng>::Seed);

impl FromStr for Seed {
    type Err = DecodeError;

    /// Parses a 64-digit hex string into an RNG seed.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut seed = Self::default();
        if HEXLOWER_PERMISSIVE.decode_len(s.len())? != seed.0.len() {
            return Err(DecodeError {
                position: s.len(),
                kind: DecodeKind::Length,
            });
        }
        match HEXLOWER_PERMISSIVE.decode_mut(s.as_bytes(), &mut seed.0) {
            Ok(_) => Ok(seed),
            Err(e) => Err(e.error),
        }
    }
}

impl fmt::Display for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&HEXLOWER_PERMISSIVE.encode(&self.0))
    }
}

impl Distribution<Seed> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Seed {
        Seed(self.sample(rng))
    }
}

impl Serialize for Seed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        HEXLOWER_PERMISSIVE.encode(&self.0).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Seed {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // FIXME: support deserialize to both `&'de str` and `String`.
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl Seed {
    /// Constructs a RNG from this seed.
    pub fn make_rng(&self) -> rand_hc::Hc128Rng {
        rand_hc::Hc128Rng::from_seed(self.0)
    }
}

/// Names of random number generators supported by `dbgen`.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RngName {
    /// ChaCha12
    ChaCha12,
    /// ChaCha20
    #[serde(alias = "chacha")]
    ChaCha20,
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
    /// Mock RNG which steps by a constant.
    Step,
}

impl FromStr for RngName {
    type Err = Error;
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(match name {
            "chacha12" => Self::ChaCha12,
            "chacha" | "chacha20" => Self::ChaCha20,
            "hc128" => Self::Hc128,
            "isaac" => Self::Isaac,
            "isaac64" => Self::Isaac64,
            "xorshift" => Self::XorShift,
            "pcg32" => Self::Pcg32,
            "step" => Self::Step,
            _ => {
                return Err(Error::UnsupportedCliParameter {
                    kind: "RNG",
                    value: name.to_owned(),
                })
            }
        })
    }
}

impl RngName {
    /// Creates an RNG engine given the name. The RNG engine instance will be seeded from `src`.
    fn create(self, src: &mut rand_hc::Hc128Rng) -> Box<dyn RngCore + Send> {
        match self {
            Self::ChaCha12 => Box::new(rand_chacha::ChaCha12Rng::from_seed(src.gen())),
            Self::ChaCha20 => Box::new(rand_chacha::ChaCha20Rng::from_seed(src.gen())),
            Self::Hc128 => Box::new(rand_hc::Hc128Rng::from_seed(src.gen())),
            Self::Isaac => Box::new(rand_isaac::IsaacRng::from_seed(src.gen())),
            Self::Isaac64 => Box::new(rand_isaac::Isaac64Rng::from_seed(src.gen())),
            Self::XorShift => Box::new(rand_xorshift::XorShiftRng::from_seed(src.gen())),
            Self::Pcg32 => Box::new(rand_pcg::Pcg32::from_seed(src.gen())),
            Self::Step => Box::new(StepRng::new(src.next_u64(), src.next_u64() | 1)),
        }
    }
}

/// Names of output formats supported by `dbgen`.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FormatName {
    /// SQL
    Sql,
    /// CSV
    Csv,
    /// SQL in INSERT-SET form
    SqlInsertSet,
}

impl FromStr for FormatName {
    type Err = Error;
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(match name {
            "sql" => Self::Sql,
            "csv" => Self::Csv,
            "sql-insert-set" => Self::SqlInsertSet,
            _ => {
                return Err(Error::UnsupportedCliParameter {
                    kind: "output format",
                    value: name.to_owned(),
                })
            }
        })
    }
}

impl FormatName {
    /// Obtains the file extension when using this format.
    fn extension(self) -> &'static str {
        match self {
            Self::Sql | Self::SqlInsertSet => "sql",
            Self::Csv => "csv",
        }
    }

    /// Creates a formatter writer given the name.
    fn create(self, options: &Options) -> Box<dyn Format + '_> {
        match self {
            Self::Sql => Box::new(SqlFormat(options)),
            Self::Csv => Box::new(CsvFormat(options)),
            Self::SqlInsertSet => Box::new(SqlInsertSetFormat(options)),
        }
    }

    #[allow(clippy::unused_self)] // future compatibility with other formats.
    fn default_true_string(self) -> Cow<'static, str> {
        Cow::Borrowed("1")
    }

    #[allow(clippy::unused_self)] // future compatibility with other formats.
    fn default_false_string(self) -> Cow<'static, str> {
        Cow::Borrowed("0")
    }

    fn default_null_string(self) -> Cow<'static, str> {
        Cow::Borrowed(match self {
            Self::Sql | Self::SqlInsertSet => "NULL",
            Self::Csv => r"\N",
        })
    }
}

/// Names of the compression output formats supported by `dbgen`.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionName {
    /// Compress as gzip format (`*.gz`).
    #[serde(alias = "gz")]
    Gzip,
    /// Compress as xz format (`*.xz`).
    Xz,
    /// Compress as Zstandard format (`*.zst`).
    #[serde(alias = "zst")]
    Zstd,
}

impl FromStr for CompressionName {
    type Err = Error;
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(match name {
            "gzip" | "gz" => Self::Gzip,
            "xz" => Self::Xz,
            "zstd" | "zst" => Self::Zstd,
            _ => {
                return Err(Error::UnsupportedCliParameter {
                    kind: "compression format",
                    value: name.to_owned(),
                })
            }
        })
    }
}

impl CompressionName {
    /// Obtains the file extension when using this format.
    fn extension(self) -> &'static str {
        match self {
            Self::Gzip => "gz",
            Self::Xz => "xz",
            Self::Zstd => "zst",
        }
    }

    /// Wraps a writer with a compression layer on top.
    fn wrap<'a, W: Write + 'a>(self, inner: W, level: u8) -> Box<dyn Write + 'a> {
        match self {
            Self::Gzip => Box::new(GzEncoder::new(inner, flate2::Compression::new(level.into()))),
            Self::Xz => Box::new(XzEncoder::new(inner, level.into())),
            Self::Zstd => Box::new(
                zstd::Encoder::new(inner, level.into())
                    .expect("valid zstd encoder")
                    .auto_finish(),
            ),
        }
    }
}

/// Names of the components to be produced `dbgen`.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[repr(u8)]
#[serde(rename_all = "lowercase")]
pub enum ComponentName {
    /// The `CREATE SCHEMA` SQL file.
    Schema = 1,
    /// The `CREATE TABLE` SQL file.
    Table = 2,
    /// The data files.
    Data = 4,
}

impl FromStr for ComponentName {
    type Err = Error;
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(match name {
            "schema" => Self::Schema,
            "table" => Self::Table,
            "data" => Self::Data,
            _ => {
                return Err(Error::UnsupportedCliParameter {
                    kind: "component",
                    value: name.to_owned(),
                })
            }
        })
    }
}

impl ComponentName {
    fn union_all(it: impl IntoIterator<Item = Self>) -> u8 {
        it.into_iter().fold(0, |mask, cn| mask | (cn as u8))
    }

    fn remove_from(self, mask: &mut u8) {
        *mask &= !(self as u8);
    }

    fn is_in(self, mask: u8) -> bool {
        mask & (self as u8) != 0
    }
}

/// A [`Writer`] which counts how many bytes are written.
struct FormatWriter<'a> {
    /// The target writer.
    writer: BufWriter<Box<dyn Write>>,
    /// Total number of bytes currently written into `writer`.
    written_size: u64,
    /// Total number of bytes written which is not yet committed into
    /// the `WRITTEN_SIZE` global variable.
    uncommitted_size: u64,
    /// The prefix part of the path.
    path_prefix: PathBuf,
    /// The extension of the path.
    path_extension: &'static str,
    /// The file size limit and the associated lexicographical counter for when
    /// size-splitting is needed.
    target_size_and_counter: Option<(u64, LexCtr)>,
    /// The output file format.
    format: &'a dyn Format,
}
impl<'a> FormatWriter<'a> {
    /// Creates a new [`WriteWrapper`].
    fn new(
        path_prefix: PathBuf,
        path_extension: &'static str,
        target_size: Option<u64>,
        format: &'a dyn Format,
    ) -> Self {
        Self {
            writer: BufWriter::with_capacity(0, Box::new(sink())),
            written_size: 0,
            uncommitted_size: 0,
            path_prefix,
            path_extension,
            target_size_and_counter: target_size.map(|s| (s, LexCtr::default())),
            format,
        }
    }

    /// Returns the current file path.
    fn path(&self) -> PathBuf {
        let mut path_prefix = self.path_prefix.as_os_str().to_owned();
        if let Some((_, counter)) = &self.target_size_and_counter {
            path_prefix.push(&counter.to_string());
        }
        path_prefix.push(".");
        path_prefix.push(self.path_extension);
        path_prefix.into()
    }

    /// Checks if the current written size exceeds the size limit.
    fn try_rotate(&mut self) -> bool {
        if let Some((size, counter)) = &mut self.target_size_and_counter {
            if self.written_size >= *size {
                counter.inc();
                self.written_size = 0;
                return true;
            }
        }
        false
    }
}

impl Write for FormatWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes_written = self.writer.write(buf)?;
        self.written_size += bytes_written as u64;
        self.uncommitted_size += bytes_written as u64;
        Ok(bytes_written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl writer::Writer for FormatWriter<'_> {
    fn write_value(&mut self, value: &Value) -> Result<(), S<Error>> {
        self.format
            .write_value(self, value)
            .with_path_fn("write value", || self.path())
    }
    fn write_file_header(&mut self, schema: &Schema<'_>) -> Result<(), S<Error>> {
        self.format
            .write_file_header(self, schema)
            .with_path_fn("write file header", || self.path())
    }
    fn write_header(&mut self, schema: &Schema<'_>) -> Result<(), S<Error>> {
        self.format
            .write_header(self, schema)
            .with_path_fn("write header", || self.path())
    }
    fn write_value_header(&mut self, column: &str) -> Result<(), S<Error>> {
        self.format
            .write_value_header(self, column)
            .with_path_fn("write value header", || self.path())
    }
    fn write_value_separator(&mut self) -> Result<(), S<Error>> {
        self.format
            .write_value_separator(self)
            .with_path_fn("write value separator", || self.path())
    }
    fn write_row_separator(&mut self) -> Result<(), S<Error>> {
        self.format
            .write_row_separator(self)
            .with_path_fn("write row separator", || self.path())
    }
    fn write_trailer(&mut self) -> Result<(), S<Error>> {
        self.format
            .write_trailer(self)
            .with_path_fn("write trailer", || self.path())
    }
}

/// The environmental data shared by all data writers.
#[allow(clippy::struct_excessive_bools)] // the booleans aren't used as state-machines.
struct Env {
    out_dir: PathBuf,
    file_num_digits: usize,
    tables: Vec<Table>,
    qualified: bool,
    rows_count: u32,
    format: FormatName,
    format_options: Options,
    compression: Option<(CompressionName, u8)>,
    components_mask: u8,
    file_size: Option<u64>,
}

/// Information specific to a file and its derived tables.
struct FileInfo {
    file_index: u32,
    inserts_count: u32,
    last_insert_rows_count: u32,
}

impl Env {
    /// Writes the `CREATE SCHEMA` schema files.
    fn write_schema_schema(&self) -> Result<(), S<Error>> {
        let mut schema_names = HashMap::with_capacity(1);
        for table in &self.tables {
            if let (Some(unique_name), Some(name)) = (table.name.unique_schema_name(), table.name.schema_name()) {
                schema_names.insert(unique_name, name);
            }
        }
        for (unique_name, name) in schema_names {
            let path = self.out_dir.join(format!("{}-schema-create.sql", unique_name));
            let mut file = BufWriter::new(File::create(&path).with_path("create schema schema file", &path)?);
            writeln!(file, "CREATE SCHEMA {};", name).with_path("write schema schema file", &path)?;
        }
        Ok(())
    }

    /// Writes the `CREATE TABLE` schema files.
    fn write_table_schema(&self) -> Result<(), S<Error>> {
        for table in &self.tables {
            let path = self.out_dir.join(format!("{}-schema.sql", table.name.unique_name()));
            let mut file = BufWriter::new(File::create(&path).with_path("create table schema file", &path)?);
            write!(
                file,
                "CREATE TABLE {} {}",
                table.name.table_name(self.qualified),
                table.content
            )
            .with_path("write table schema file", &path)?;
        }
        Ok(())
    }

    fn open_data_file(&self, path: PathBuf) -> Result<Box<dyn Write>, S<Error>> {
        Ok(if !ComponentName::Data.is_in(self.components_mask) {
            Box::new(sink())
        } else if let Some((compression, level)) = self.compression {
            let mut path = path.into_os_string();
            path.push(".");
            path.push(compression.extension());
            let path = PathBuf::from(path);
            compression.wrap(File::create(&path).with_path("create data file", &path)?, level)
        } else {
            Box::new(File::create(&path).with_path("create data file", &path)?)
        })
    }

    /// Writes the data file.
    fn write_data_file(&self, info: &FileInfo, state: &mut State) -> Result<(), S<Error>> {
        let path_suffix = format!(".{0:01$}", info.file_index, self.file_num_digits);
        let format = self.format.create(&self.format_options);

        let mut fwe = writer::Env::new(&self.tables, state, self.qualified, |table| {
            let path = self.out_dir.join([table.name.unique_name(), &path_suffix].concat());
            let mut w = FormatWriter::new(path, self.format.extension(), self.file_size, &*format);
            w.writer = BufWriter::new(self.open_data_file(w.path())?);
            Ok(w)
        })?;

        for i in 0..info.inserts_count {
            let rows_count = if i == info.inserts_count - 1 {
                info.last_insert_rows_count
            } else {
                self.rows_count
            };
            for _ in 0..rows_count {
                fwe.write_row()?;
            }
            fwe.write_trailer()?;

            let mut total_uncommitted_size = 0;
            for (table, w) in fwe.tables() {
                total_uncommitted_size += mem::take(&mut w.uncommitted_size);
                if w.try_rotate() {
                    let new_path = w.path();
                    w.writer.flush().with_path("flush old file for rotation", &new_path)?;
                    w.writer = BufWriter::new(self.open_data_file(new_path)?);
                    w.write_file_header(&table.schema(self.qualified))?;
                }
            }
            WRITTEN_SIZE.fetch_add(total_uncommitted_size, Ordering::Relaxed);
            WRITE_PROGRESS.fetch_add(rows_count.into(), Ordering::Relaxed);
        }
        Ok(())
    }
}

/// Runs the progress bar thread.
///
/// This function will loop and update the progress bar every 0.5 seconds, until [`WRITE_FINISHED`]
/// becomes `true`.
fn run_progress_thread(total_rows: u64) {
    #[allow(clippy::non_ascii_literal)]
    const TICK_FORMAT: &str = "🕐🕑🕒🕓🕔🕕🕖🕗🕘🕙🕚🕛";

    let mb = MultiBar::new();

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
        let rows_count = WRITE_PROGRESS.load(Ordering::Relaxed);
        pb.set(rows_count);

        let written_size = WRITTEN_SIZE.load(Ordering::Relaxed);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_args() {
        let test_cases = vec![
            (
                Args {
                    files_count: 11,
                    inserts_count: 181,
                    rows_count: 97,
                    ..Args::default()
                },
                RowArgs {
                    files_count: 11,
                    inserts_count: 181,
                    last_file_inserts_count: 181,
                    rows_count: 97,
                    final_insert_rows_count: 97,
                    last_file_final_insert_rows_count: 97,
                    rows_per_file: 17_557,
                    total_count: 193_127,
                },
            ),
            (
                Args {
                    files_count: 11,
                    inserts_count: 181,
                    rows_count: 97,
                    last_file_inserts_count: Some(151),
                    last_insert_rows_count: Some(53),
                    ..Args::default()
                },
                RowArgs {
                    files_count: 11,
                    inserts_count: 181,
                    last_file_inserts_count: 151,
                    rows_count: 97,
                    final_insert_rows_count: 97,
                    last_file_final_insert_rows_count: 53,
                    rows_per_file: 17_557,
                    total_count: 190_173,
                },
            ),
            (
                Args {
                    files_count: 11,
                    rows_per_file: Some(18_013),
                    rows_count: 97,
                    ..Args::default()
                },
                RowArgs {
                    files_count: 11,
                    inserts_count: 186,
                    last_file_inserts_count: 186,
                    rows_count: 97,
                    final_insert_rows_count: 68,
                    last_file_final_insert_rows_count: 68,
                    rows_per_file: 18_013,
                    total_count: 198_143,
                },
            ),
            (
                Args {
                    files_count: 11,
                    rows_per_file: Some(17_557),
                    rows_count: 97,
                    last_file_inserts_count: Some(151),
                    last_insert_rows_count: Some(53),
                    ..Args::default()
                },
                RowArgs {
                    files_count: 11,
                    inserts_count: 181,
                    last_file_inserts_count: 151,
                    rows_count: 97,
                    final_insert_rows_count: 97,
                    last_file_final_insert_rows_count: 53,
                    rows_per_file: 17_557,
                    total_count: 190_173,
                },
            ),
            (
                Args {
                    total_count: Some(190_173),
                    rows_per_file: Some(17_557),
                    rows_count: 97,
                    ..Args::default()
                },
                RowArgs {
                    files_count: 11,
                    inserts_count: 181,
                    last_file_inserts_count: 151,
                    rows_count: 97,
                    final_insert_rows_count: 97,
                    last_file_final_insert_rows_count: 53,
                    rows_per_file: 17_557,
                    total_count: 190_173,
                },
            ),
            (
                Args {
                    total_count: Some(198_143),
                    rows_per_file: Some(18_013),
                    rows_count: 97,
                    ..Args::default()
                },
                RowArgs {
                    files_count: 11,
                    inserts_count: 186,
                    last_file_inserts_count: 186,
                    rows_count: 97,
                    final_insert_rows_count: 68,
                    last_file_final_insert_rows_count: 68,
                    rows_per_file: 18_013,
                    total_count: 198_143,
                },
            ),
            (
                Args {
                    total_count: Some(199_909),
                    rows_per_file: Some(18_013),
                    rows_count: 97,
                    ..Args::default()
                },
                RowArgs {
                    files_count: 12,
                    inserts_count: 186,
                    last_file_inserts_count: 19,
                    rows_count: 97,
                    final_insert_rows_count: 68,
                    last_file_final_insert_rows_count: 20,
                    rows_per_file: 18_013,
                    total_count: 199_909,
                },
            ),
        ];

        for (args, row_args) in test_cases {
            assert_eq!(args.row_args(), row_args);
        }
    }
}
