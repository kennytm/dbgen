Template reference
==================

File syntax
-----------

The template file should consist of one CREATE TABLE AS statement, like this:

```sql
CREATE TABLE "database"."schema"."table" (
    column_1    COLUMN_TYPE_1,
    column_2    COLUMN_TYPE_2,
    -- ...
    column_n    COLUMN_TYPE_N
) OPTION_1 = 1, /*...*/ OPTION_N = N
AS SELECT
    rand.int(16),
    rand.regex('[a-z]*'),
    -- ...
    rand.uniform(-4.0, 4.0)
FROM rand;
```

(The `FROM rand` at the end is part

Expression syntax
-----------------

Each value in the SELECT clause can be an expression. `dbgen` will evaluate the expression to
generate a new row when writing them out.

### Literals

`dbgen` supports integer, float and string literals.

* **Integers**

    Decimal and hexadecimal numbers are supported. The value must be between 0 and
    2<sup>64</sup> − 1.

    Examples: `0`, `3`, `18446744073709551615`, `0X1234abcd`, `0xFFFFFFFFFFFFFFFF`

* **Floating point numbers**

    Numbers will be stored in IEEE-754 double-precision format.

    Examples: `0.0`, `1.5`, `.5`, `2.`, `1e100`, `1.38e-23`, `6.02e+23`

* **Strings**

    Strings must be encoded as UTF-8, and written between single quotes (double-quoted strings are
    *not* supported). To represent a single quote in the string, use `''`.

    Examples: `'Hello'`, `'10 o''clock'`

### Operators

From highest to lowest precedence:

* **-x**

    Negative of *x*. The number *x* must be an integer or float.

### Symbols

* **rownum**

    The current row number. The first row has value 1.

### Random functions

* **rand.int(32)**

    Generates a uniform random signed integer with the given number of bits (must be between 1 and
    64).

* **rand.uint(32)**

    Generates a uniform random unsigned integer with the given number of bits (must be between 1 and
    64).

* **rand.regex('[0-9a-z]+', 'i', 100)**

    Generates a random string satisfying the regular expression. The second and third parameters are
    optional. If provided, they specify respectively the regex flags, and maximum repeat count for
    the unbounded repetition operators (`+`, `*` and `{n,}`).

    The input string should satisfy the syntax of the Rust regex package. The flags is a string
    composed of these letters:

    * `x` (ignore whitespace)
    * `i` (case insensitive)
    * `s` (dot matches new-line)
    * `u` (enable Unicode mode)
    * `a` (disable Unicode mode)
    * `o` (recognize octal escapes)

    The flags `m` (multi-line) and `U` (ungreedy) does not affect string generation and are ignored.

* **rand.range(7, 19)**

    Generates a random integer uniformly distributed in the half-open interval 7 ≤ *x* < 19.
    The length of the range must be less than 2<sup>64</sup>.

* **rand.range_inclusive(8, 35)**

    Generates a random integer uniformly distributed in the closed interval 8 ≤ *x* ≤ 35.
    The length of the range must be less than 2<sup>64</sup>.

* **rand.uniform(2.4, 7.5)**

    Generates a random floating point number uniformly distributed in the half-open interval
    2.4 ≤ *x* < 7.5.

* **rand.uniform_inclusive(1.6, 8.4)**

    Generates a random floating point number uniformly distributed in the closed interval
    1.6 ≤ *x* ≤ 8.4.
