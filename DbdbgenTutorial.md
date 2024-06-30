`dbdbgen` Tutorial
==================

In this example, we will to create a `dbdbgen` program to generate a table with
M columns and N rows populated with 0 or 1 randomly.

`dbdbgen` programs are written in [Jsonnet](https://jsonnet.org/). We strongly
recommend learning the Jsonnet language before continuing. You should also be
familiar with the `dbgen` [template](./Template.md) language.

## Getting started

We first create a Jsonnet file filled with basic information about the program:

```jsonnet
// rand01.jsonnet
{
    name: 'rand01',
    version: '0.1.0',
    about: 'Just a sample.',
}
```

`dbdbgen` is able to recognize this basic skeleton. Its help screen shows the
program's name, version and description:

```console
$ dbdbgen rand01.jsonnet --help
Just a sample.

Usage: dbdbgen rand01

Options:
  -h, --help
          Print help
  -V, --version
          Print version
```

## First step

In a `dbdbgen` program, we place the instructions into the **steps** field.

```jsonnet
{
    steps: [],
}
```

Let's start by producing a table with 1 column and 1 row. The `dbgen` template
is:

```sql
CREATE TABLE rand01 (
    col1 integer /*{{ rand.range_inclusive(0, 1) }}*/
);
```

In a `dbdbgen` program, we put this as a string into the **template_string**
field in a **steps** item. The output directory can also be placed into
**out_dir** field:

```jsonnet
{
    steps: [
        {
            out_dir: 'rand01',
            template_string: |||
                CREATE TABLE rand01 (
                    col1 integer /*{{ rand.range_inclusive(0, 1) }}*/
                );
            |||,
        },
    ],
}
```

Running this will produce the `./rand01` directory, containing the SQL dump of
this 1×1 table.

```console
$ dbdbgen rand01.jsonnet
step 1 / 1
Using seed: f376c0d7eb308d19858fe3286cc8900fe3a50bbd3f1daafab9f80cec72b5e22d
Done!
Size     31 B / 31 B 🕒  61 B/s

$ ls rand01
rand01.1.sql       rand01-schema.sql

$ cat rand01/rand01.1.sql
INSERT INTO rand01 VALUES
(0);
```

## Simple arguments

It is a bad idea to hard-code the output directory into the program. `dbdbgen`
thus allows programs to accept additional command line arguments through the
**args** field.

```jsonnet
{
    args: {},
    steps: [],
}
```

Here, we require the user to provide an `--out-dir`/`-o` argument:

```jsonnet
{
    name: 'rand01',
    version: '0.2.0',
    about: 'Demonstrating the --out-dir flag.',
    args: {
        out_dir: {
            short: 'o',         // we accept '-o'
            long: 'out-dir',    // we accept '--out-dir'
            help: 'Output directory.',
            required: true,     // the argument must be provided
            type: 'str',        // the argument returns a string
        },
    },
    steps: [],
}
```

If we run the program without `--out-dir`/`-o`, an error would occur. We can
also check the updated help screen.

```console
$ dbdbgen rand01.jsonnet
error: the following required arguments were not provided:
  --out-dir <out_dir>

Usage: dbdbgen rand01 --out-dir <out_dir>

For more information, try '--help'.

$ dbdbgen rand01.jsonnet --help
Demonstrating the --out-dir flag.

Usage: dbdbgen rand01 --out-dir <out_dir>

Options:
  -o, --out-dir <out_dir>
          Output directory.
  -h, --help
          Print help
  -V, --version
          Print version
```

When the arguments are defined, the **steps** should be defined as a function to
accept the result:

```jsonnet
{
    name: 'rand01',
    version: '0.2.1',
    about: 'Demonstrating the --out-dir flag.',
    args: {
        out_dir: {
            short: 'o',
            long: 'out-dir',
            help: 'Output directory.',
            required: true,
            type: 'str',
        },
    },

    steps(matches):: [                  // matches is a map of CLI matches.
        {
            out_dir: matches.out_dir,   // corresponds to the "out_dir" key in args
            template_string: |||
                CREATE TABLE rand01 (
                    col1 integer /*{{ rand.range_inclusive(0, 1) }}*/
                );
            |||,
        }
    ],
}
```

Now we can use the `-o` flag to set the output directory.

```console
$ dbdbgen rand01.jsonnet -o rand01_2
step 1 / 1
Using seed: 58a5ca50c2b1abd3b8006524d74304ab65fd04750dbe4ff624bc057b30c71ed1
Done!
Size     31 B / 31 B 🕒  62 B/s

$ ls rand01_2/
rand01.1.sql  rand01-schema.sql
```

## Standard arguments

`dbgen` has a lot of configuration flags, it would be annoying to define them
manually in every `dbdbgen` program. Therefore, `dbdbgen` provides a
[supplemental library "`dbdbgen.libsonnet`"](dbdbgen/dbdbgen.libsonnet) with a
predefined set of standard arguments.

```jsonnet
local dbdbgen = import 'dbdbgen.libsonnet';  // import the supplemental library
{
    name: 'rand01',
    version: '0.3.0',
    about: 'Demonstrating the standard arguments',
    args: dbdbgen.stdArgs,                   // use the standard dbgen arguments.
    steps: [],
}
```

The matches returned by `dbdbgen.stdArgs` are compatible with **steps** items
and can be used directly.

```jsonnet
local dbdbgen = import 'dbdbgen.libsonnet';
{
    name: 'rand01',
    version: '0.3.1',
    about: 'Demonstrating the standard arguments',
    args: dbdbgen.stdArgs,
    steps(matches): [
        matches {  // just add the template_string field to the matches.
            template_string: |||
                CREATE TABLE rand01 (
                    col1 integer /*{{ rand.range_inclusive(0, 1) }}*/
                );
            |||,
        },
    ],
}
```

We can then use the standard `dbgen` CLI arguments like `--total-count`/`-N` and `--rows-per-file`/`-R` in this custom program:

```console
$ dbdbgen rand01.jsonnet -h
dbdbgen-rand01 0.3.1
Demonstrating the standard arguments

Usage: dbdbgen rand01 [OPTIONS] --out-dir <out_dir>

Options:
      --components <components>
          Components to write. [default: table,data] [possible values: schema, table, data]

— « skipped » —

  -N, --total-count <total_count>
          Total number of rows of the main table.
  -h, --help
          Print help
  -V, --version
          Print version

$ dbdbgen rand01.jsonnet -o rand01_3 -N 3 -R 3 -f csv
step 1 / 1
Using seed: 12f1f00389a3034ad192e27dcec5631353b29cc18f6744b774b97051d2c868a1
Done!
Size     6 B / 6 B 🕒  12 B/s

$ ls rand01_3/
rand01.1.csv  rand01-schema.sql

$ cat rand01_3/rand01.1.csv
1
1
0
```

## Final result

Finally, we construct the program for the original purpose: generates a table
with M columns and N rows. We want the user to provide the number M:

```jsonnet
local dbdbgen = import 'dbdbgen.libsonnet';
{
    name: 'rand01',
    version: '0.4.0',
    about: 'Generates an M×N table.',
    args: dbdbgen.stdArgs {
        columns_count: {
            short: 'M',
            long: 'columns-count',
            help: 'Number of columns.',
            required: true,
            type: 'int',
        },
    },
    steps: [],
}
```

We can also modify the standard argument. For instance, we want to
* Make `--total-count`/`-N` required too, instead of the default 1
* Make `--rows-per-file`/`-R` default to a larger number, instead of the default 1
* Disable irrelevant flags like `--escape-backslash` and `--now`.

The outcome is this:

```jsonnet
local dbdbgen = import 'dbdbgen.libsonnet';
{
    name: 'rand01',
    version: '0.4.1',
    about: 'Generates an M×N table.',
    args: dbdbgen.stdArgs {
        columns_count: {
            short: 'M',
            long: 'columns-count',
            help: 'Number of columns.',
            required: true,
            type: 'int',
        },

        // modify the existing arguments with the `+:` syntax
        total_count+: { required: true },
        rows_per_file+: { default: '1000' },

        // remove existing arguments by hiding them
        escape_backslash:: null,
        now:: null,
    },
    steps: [],
}
```

Finally, fill in the step:

```jsonnet
local dbdbgen = import 'dbdbgen.libsonnet';
{
    name: 'rand01',
    version: '0.4.2',
    about: 'Generates an M×N table.',
    args: dbdbgen.stdArgs {
        columns_count: {
            short: 'M',
            long: 'columns-count',
            help: 'Number of columns.',
            required: true,
            type: 'int',
        },
        total_count+: { required: true },
        rows_per_file+: { default: '1000' },
        escape_backslash:: null,
        now:: null,
    },

    steps(m):
        local col_fmt = 'col%d integer /*{{ rand.range_inclusive(0, 1) }}*/';
        local columns = [col_fmt % i for i in std.range(1, m.columns_count)];
        local template = 'CREATE TABLE rand01(%s);' % std.join(',', columns);
        [ m { template_string: template } ],
}
```

Execute this to get our desired result:

```console
$ dbdbgen rand01.jsonnet -M 6 -N 5 -o rand01_4 -f csv
step 1 / 1
Using seed: 394dd95f85ccea9f2c001a8cf3df6732dc05c24ff649462e6586bc5d76c40269
Done!
Size     60 B / 60 B 🕒  120 B/s

$ ls rand01_4/
rand01.1.csv  rand01-schema.sql

$ cat rand01_4/rand01-schema.sql
CREATE TABLE rand01 (col1 integer,col2 integer,col3 integer,col4 integer,col5 integer,col6 integer);

$ cat rand01_4/rand01.1.csv
0,0,0,0,1,1
0,0,1,0,1,0
0,0,1,0,0,1
0,1,1,1,0,1
1,0,0,1,1,0
```
