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

* `--table-name «NAME»`

    Override the table name of generated data. Should be a qualified and quoted name like
    `'"database"."schema"."table"'`.

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
    | `pcg32`           | [PCG32-Oneseq]    |
    | `xoshiro256**`    | [xoshiro256**]    |

* `-j «N»`, `--jobs «N»`

    Use *N* threads to write the output in parallel. Default to the number of logical CPUs.

* `-q`, `--quiet`

    Disable progress bar output.

* `--escape-backslash`

    When enabled, backslash (`\\`) is considered introducing a C-style escape sequence, and should
    itself be escaped as `\\\\`. In standard SQL, the backslash does not have any special meanings.
    This setting should match that of the target database, otherwise it could lead to invalid data
    or syntax error if a generated string contained a backslash.

    | SQL dialect | Should pass `--escape-backslash`                                |
    |-------------|-----------------------------------------------------------------|
    | MySQL       | Yes if [`NO_BACKSLASH_ESCAPES`] is off (default)                |
    | PostgreSQL  | No if [`standard_conforming_strings`] is on (default since 9.1) |
    | SQLite3     | No                                                              |
    | TransactSQL | No                                                              |

[ChaCha20]: https://cr.yp.to/chacha.html
[HC-128]: https://www.ntu.edu.sg/home/wuhj/research/hc/index.html
[ISAAC]: http://www.burtleburtle.net/bob/rand/isaacafa.html
[Xorshift]: https://en.wikipedia.org/wiki/Xorshift
[PCG32-Oneseq]: http://www.pcg-random.org/
[xoshiro256**]: http://xoshiro.di.unimi.it/

[`NO_BACKSLASH_ESCAPES`]: https://dev.mysql.com/doc/refman/8.0/en/sql-mode.html#sqlmode_no_backslash_escapes
[`standard_conforming_strings`]: https://www.postgresql.org/docs/current/static/runtime-config-compatible.html#GUC-STANDARD-CONFORMING-STRINGS