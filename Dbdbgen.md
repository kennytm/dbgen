`dbdbgen` Reference
===================

```sh
dbdbgen res/tpcc.jsonnet -w 50 -o ./tpcc_out
```

`dbdbgen` is a metaprogram generating a random database of many tables through
multiple invocation of `dbgen`. It can be used to populate a set of related
tables such as those used in TPC-C and TPC-H benchmarks.

Usage
-----

```sh
dbdbgen [--dry-run] [--allow-import] program.jsonnet ...
```

* `program.jsonnet ...`

    The program describing how the database is generated, followed by the
    arguments passed into the program.

* `--dry-run`

    Runs the program without running `dbgen`. The evaluated steps are printed to
    stdout as JSON with comment, which can in turn be used again as a program.

* `--allow-import`

    Allows the `import` and `importstr` constructs to read from the file system.
    The default is false, meaning only `import 'dbdbgen.libsonnet'` is allowed.

The input to `dbdbgen` is a [Jsonnet](https://jsonnet.org/) file, specifying the
command line interface and what arguments to be passed into `dbgen`. The
workflow is like this:

1. First, it evaluates `$.args` to construct a command line parser, consuming
    the remaining arguments passed into `dbdbgen`.
2. Then, it evaluates `$.steps(m)` where `m` is the matches from CLI, and
    produces a list of instructions.
3. Finally, it executes those `dbgen` commands sequentially.

Specification
-------------

The content of the Jsonnet file should be typed like `Spec` below.

```typescript
type Spec = {
    steps: ((matches: {[key: string]: Match}) => Step[]) | Step[],

    name: string,
    version: string,
    about: string,
    args: {[key: string]: Arg},
};
```

### Arg

```typescript
type Arg = {
    short: string,
    long: string,
    help: string,
    required: boolean,
    default: string | null,
    type: 'bool' | 'str' | 'int' | 'size' | {choices: {choices: string[], multiple: boolean}}
};
```

* **short** (default: `''`)

    Short name of the argument, e.g. `short: 'f'` means the CLI accepts `-f`.

    If this field is empty, the argument does not have any short name.

* **long** (default: *key*)

    Long name of the argument, e.g. `long: 'output-format'` means the CLI
    accepts `--output-format`.

    If this field is absent, the key used to introduced the argument is taken as
    the long name, e.g. in

    ```js
    {
        args: {
            example: {          // <-- key = 'example'
                help: '...'
            }
        }
    }
    ```

    the argument "example" implicitly contains `long: 'example'`, meaning the
    CLI accepts `--example`.

* **help** (default: `''`)

    Human readable description of the argument shown in the `--help` screen.

* **required** (default: `false`)

    Whether this argument is required. If set to true, the user must provide
    this argument or otherwise `dbdbgen` will exit.

* **default** (default: `null`)

    The default input to use if the argument is absent. It represents the user
    input and thus must be a string regardless of the output type.

    This field is ignored when `required: true` or `type: 'bool'`.

* **type** (default: `'str'`)

    The output type. Should be one of:

    | Value    | Description |
    |----------|-------------|
    | `'bool'` | The argument is a flag, taking no input (present = true, absent = false). |
    | `'str'`  | The argument is an arbitrary string. |
    | `'int'`  | The argument is an unsigned decimal integer. Errors on non-integer. |
    | `'size'` | The argument is an unsigned integer for file byte size (e.g. `1 MiB`). Errors on non-integer. |
    | `'float'` | The argument is a floating point number. Errors on non-number. |
    | `{choices:…}` | The argument must be selected from the strings listed in the choices. |

    The function `dbdbgen.choices(['x', 'y'], multiple=b)` is equivalent
    to the object `{choices: {choices: ['x', 'y'], multiple: b}}`. Using the
    function is recommended.

    When **choices**.**multiple** is false, the output is a string. When
    **choices**.**multiple** is true, the output is a string array.

Besides the **args** field, the **name**, **version** and **about** fields
provide additional human-readable description of the program. They are shown in
the `--help` screen.

### Match

```typescript
type Match = boolean | string | number | string[];
```

The **steps** field can either by an array of steps, or a function returning an
array of steps. The function takes a map of "matches".

The map keys form a subset of the keys of **args**. When the user did not
provide an argument and it has no default value, the key will be missing from
the matches.

A special treatment is made for the `seed` key. If the `seed` key is missing
from the matches, `dbdbgen` will generate a random 64-digit hex string and add
to the final matches. This gives the program a source of randomness always.

### Step

```typescript
interface Step {
    qualified: boolean,
    table_name: string | null,
    schema_name: string | null,
    out_dir: string,
    total_count: number,
    rows_per_file: number,
    size: number | null,
    escape_backslash: boolean,
    template_string: string,
    seed: string | null,
    jobs: number,
    rng: 'chacha12' | 'chacha20' | 'hc128' | 'isaac' | 'isaac64' | 'xorshift' | 'pcg32' | 'step',
    quiet: boolean,
    now: string | null,
    format: 'sql' | 'csv' | 'sql-insert-set',
    format_true: string | null,
    format_false: string | null,
    format_null: string | null,
    headers: boolean,
    compression: 'gzip' | 'xz' | 'zstd' | null,
    compress_level: number,
    components: ('schema' | 'table' | 'data')[],
    initialize: string[],
}
```

Each step describes the arguments sent to `dbgen`. They correspond to the
[`dbgen` CLI arguments](CLI.md).

| Field | `dbgen` CLI argument | Default value |
|-------|----------------------|---------------|
| qualified | `--qualified` | false |
| table_name | `--table-name` | null |
| schema_name | `--schema-name` | null |
| out_dir | `-o`/`--out-dir` | **required** |
| total_count | `-N`/`--total-count` | 1 |
| rows_per_file | `-R`/`--rows-per-file` | 1 |
| size | `-z`/`--size` | null |
| escape_backslash | `--escape-backslash` | false |
| template_string | `-e`/`--template-string` | **required** |
| seed | `-s`/`--seed` | null |
| jobs | `-j`/`--jobs` | 0 |
| rng | `--rng` | 'hc128' |
| quiet | `-q`/`--quiet` | false |
| now | `--now` | null |
| format | `-f`/`--format` | 'sql' |
| format_true | `--format-true` | null |
| format_false | `--format-false` | null |
| format_null | `--format-null` | null |
| headers | `--headers` | false |
| compression | `-c`/`--compression` | null |
| compress_level | `--compress-level` | 6 |
| components | `--components` | ['table', 'data'] |
| initialize | `-D`/`--initialize` | [] |

Supplemental library
--------------------

Besides the standard Jsonnet library (`std`), `dbdbgen` also bundles with a
[supplemental library](dbdbgen/dbdbgen.libsonnet) which can be imported with

```jsonnet
local dbdbgen = import 'dbdbgen.libsonnet';
```

The name `'dbdbgen.libsonnet'` always refer this built-in library. Even if a
file with this name exists in the local file system and `--allow-import` is
enabled, `dbdbgen` will still read the built-in one instead.

The library currently consists of the following fields:

* `dbdbgen.stdArgs`

    The standard `dbgen`-compatible arguments that can be used as the **args**
    field in the program.

    Note that this is a field, not a function.

* `dbdbgen.choices(choices, multiple=false)`

    Produces a value used in **args[].type**, representing an argument taking
    value from one of the given choices.

* `dbdbgen.xorSeed(seed, salt)`

    Given two strings of hex-digits, computes their bitwise-XOR. Example:

    ```jsonnet
    std.assertEqual(dbdbgen.xorSeed('1234abcd', '1357fedc'), '01635511')
    ```

* `dbdbgen.sha256(s)`

    Computes the SHA-256 hash of a string. Example:

    ```jsonnet
    std.assertEqual(
        dbdbgen.sha256('dbgen'),
        'c069fb143dccd2e66d526e631d13d8511934a34f1cf4df95f0137ffe2d8287a8')
    ```
