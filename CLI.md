CLI usage
=========

```sh
dbgen -i template.sql -o out_dir -k 25 -n 3000 -r 100
```

Common options
--------------

* `-i «PATH»`, `--template «PATH»`

    The path to the template file. See [Template reference](Template.md) for details.

* `-o «DIR»`, `--out-dir «DIR»`

    The directory to store the generated files. If the directory does not exist, `dbgen` will try to
    create it.

* `-k «N»`, `--files-count «N»`

    Number of data files to generate.

* `-n «N»`, `--inserts-count «N»`

    Number of INSERT statements per file to generate.

* `-r «N»`, `--rows-count «N»`

    Number of rows per INSERT statements to generate.

The total number of rows generated will be (files-count) × (inserts-count) × (rows-count).

More options
------------

* `-t «NAME»`, `--table-name «NAME»`

    Override the table name of generated data. Should be a qualified and quoted name like
    `'"database"."schema"."table"'`.

    This option cannot be used when the template has multiple tables.

* `--schema-name «NAME»`

    Replaces the schema name of the generated tables. Should be a qualified and quoted name like
    `'"database"."schema"'`.

* `--qualified`

    If specified, the generated INSERT statements will use the fully qualified table name (i.e.
    `INSERT INTO "db"."schema"."table" VALUES …`. Otherwise, only the table name will be included
    (i.e. `INSERT INTO "table" VALUES …`).

* `-s «SEED»`, `--seed «SEED»`

    Provide a 64-digit hex number to seed the random number generator, so that the output becomes
    reproducible. If not specified, the seed will be obtained from the system entropy.

    (Note: There is no guarantee that the same seed will produce the same output across major
    versions of `dbgen`.)

* `--rng «RNG»`

    Choose a random number generator. The default is `hc128` which should be the best in most
    situations. Supported alternatives are:

    | RNG name          | Algorithm         |
    |-------------------|-------------------|
    | `chacha`          | [ChaCha20]        |
    | `hc128`           | [HC-128]          |
    | `isaac`           | [ISAAC]           |
    | `isaac64`         | [ISAAC-64][ISAAC] |
    | `xorshift`        | [Xorshift]        |
    | `pcg32`           | [PCG32]           |
    | `step`            | Step sequence     |

* `-j «N»`, `--jobs «N»`

    Use *N* threads to write the output in parallel. Default to the number of logical CPUs.

* `-q`, `--quiet`

    Disable progress bar output.

* `--escape-backslash`

    When enabled, backslash (`\`) is considered introducing a C-style escape sequence, and should
    itself be escaped as `\\`. In standard SQL, the backslash does not have any special meanings.
    This setting should match that of the target database, otherwise it could lead to invalid data
    or syntax error if a generated string contained a backslash.

    | SQL dialect | Should pass `--escape-backslash`                                |
    |-------------|-----------------------------------------------------------------|
    | MySQL       | Yes if [`NO_BACKSLASH_ESCAPES`] is off (default)                |
    | PostgreSQL  | No if [`standard_conforming_strings`] is on (default since 9.1) |
    | SQLite3     | No                                                              |
    | TransactSQL | No                                                              |

* `--last-file-inserts-count «N»`

    In the last data file, generate *N* INSERT statements instead of the value
    given by `--inserts-count`.

* `--last-insert-rows-count «N»`

    In the last INSERT statement of the last data file, generate *N* rows instead of the value
    given by `--rows-count`.

    These two options allow one to fine-tune the actual rows count of the table.

* `--time-zone «TZ»`

    The time zone used to parse and format timestamps. Defaults to `UTC`, regardless of system time
    zone. Any `tz` database time zone name (e.g. `America/New_York`) can be used.

* `--now «TIMESTAMP»`

    Override the timestamp reported by `current_timestamp`. Defaults to the time when `dbgen` was
    started. The timestamp must be written in the format `YYYY-mm-dd HH:MM:SS.fff`, and it is always
    in UTC, regardless of the `--time-zone` setting.

* `-D «EXPR»`, `--initialize «EXPR»`

    Executes the global expression before generating files. This parameter can be specified multiple
    times. The expressions are executed before the global expressions in the template file. This
    allows user to parametrize the template, e.g. here we define a `@level` variable defaults to 1

    ```sql
    /*{{ @level := coalesce(@level, 1) }}*/
    create table foo (
        …
    ```

    and then we can use `-D` to override `@level`:

    ```sh
    ./dbgen -D '@level := 2' …
    ```

* `-f «FORMAT»`, `--format «FORMAT»`

    Output format. Could be one of:

    * `sql`
    * `csv`

* `-c «ALG»`, `--compress «ALG»` / `--compress-level «LEVEL»`

    Compress the data output. Possible algorithms are:

    | Algorithm | Levels |
    |-----------|--------|
    | [gzip]    | 0–9    |
    | [xz]      | 0–9    |
    | [zstd]    | 1–21   |

    The compression level defaults to 6 if not specified.

    Since the data are randomly generated, the compression ratio is typically not very high (around
    70% of uncompressed input). We do not recommend using the algorithm "xz" here, nor using very
    high compression levels.

* `--no-schemas`

    Do not generate any `CREATE TABLE` SQL files.

[ChaCha20]: https://cr.yp.to/chacha.html
[HC-128]: https://www.ntu.edu.sg/home/wuhj/research/hc/index.html
[ISAAC]: http://www.burtleburtle.net/bob/rand/isaacafa.html
[Xorshift]: https://en.wikipedia.org/wiki/Xorshift
[PCG32]: http://www.pcg-random.org/
[gzip]: https://en.wikipedia.org/wiki/Gzip
[xz]: https://en.wikipedia.org/wiki/Xz
[zstd]: https://facebook.github.io/zstd/

[`NO_BACKSLASH_ESCAPES`]: https://dev.mysql.com/doc/refman/8.0/en/sql-mode.html#sqlmode_no_backslash_escapes
[`standard_conforming_strings`]: https://www.postgresql.org/docs/current/static/runtime-config-compatible.html#GUC-STANDARD-CONFORMING-STRINGS