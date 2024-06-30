Template reference
==================

File syntax
-----------

The template file should consist of one CREATE TABLE statement, with expressions telling how a value
should be generated inside `{{ … }}` or `/*{{ … }}*/` blocks, e.g.

```sql
CREATE TABLE "database"."schema"."table" (
    "id"        INTEGER,
        /*{{ rownum }}*/
    "name"      CHAR(40),
        /*{{ rand.regex('[a-zA-Z ]{40}') }}*/
    UNIQUE KEY "some_index"("id")
);
```

Each `{{ … }}` block appearing alongside the column definitions represent one value in every
generated row. The example above may produce output like

```sql
INSERT INTO "table" VALUES
(1, 'cHvkcFSMq YbERjjeBUzLaOG TOYvDrHhfymyQeP'),
(2, 'kBAQlctdPLAlYZPyvYoBRIhYiEtOONCZQVpxpbbw'),
(3, 'ILqyqKYi jTqXaUjsgdFqYxFasnUMzXaFRvqJDdx'),
…
```

See [Advanced template features](./TemplateAdvanced.md) for more syntactical features.

Expression syntax
-----------------

`dbgen` supports an SQL-like expression syntax. These expressions will be re-evaluated for every new
row generated.

### Literals

`dbgen` supports the following literals.

* **Integers**

    Decimal and hexadecimal numbers are supported. The value must be between 0 and
    2<sup>64</sup> − 1.

    Examples: `0`, `3`, `18446744073709551615`, `0X1234abcd`, `0xFFFFFFFFFFFFFFFF`

* **Floating point numbers**

    Numbers will be stored in IEEE-754 double-precision format.

    Examples: `0.0`, `1.5`, `.5`, `2.`, `1e100`, `1.38e-23`, `6.02e+23`

* **Booleans**

    `TRUE` and `FALSE`.

    In most cases, a boolean type is converted to an integer (FALSE = 0, TRUE = 1). The only
    exception is when a boolean result is printed to the data file, which the user can change the
    output from 0/1 using the `--format-false`/`--format-true` CLI parameters.

* **Strings**

    Strings must be encoded as UTF-8, and written between single quotes (double-quoted strings are
    *not* supported). To represent a single quote in the string, use `''`.

    Examples: `'Hello'`, `'10 o''clock'`

### Operators

From highest to lowest precedence:

1. function call, array subscript `x[i]`
2. unary `-`, `+`, `~`
3. `*`, `/`
4. `+`, `-`, `||`
5. `&`
6. `|`, `^`
7. `=`, `<>`, `<`, `>`, `<=`, `>=`, `IS`, `IS NOT`
8. unary `NOT`
9. `AND`
10. `OR`
11. `:=`
12. `;`

| LHS type  | Op    | RHS type  | Result type   |
|-----------|-------|-----------|---------------|
|           | `-`   | Number    | Number        |
|           | `-`   | Interval  | Interval      |
|           | `+`   | any       | any           |
|           | `~`   | Number    | Number        |
| Number    | `*`   | Number    | Number        |
| Number    | `*`   | Interval  | Interval      |
| Interval  | `*`   | Number    | Interval      |
| Number    | `/`   | Number    | Number        |
| Interval  | `/`   | Interval  | Number        |
| Interval  | `/`   | Number    | Interval      |
| Number    | `+`   | Number    | Number        |
| Timestamp | `+`   | Interval  | Timestamp     |
| Interval  | `+`   | Timestamp | Timestamp     |
| Interval  | `+`   | Interval  | Interval      |
| Number    | `-`   | Number    | Number        |
| Timestamp | `-`   | Interval  | Timestamp     |
| Timestamp | `-`   | Timestamp | Interval      |
| Interval  | `-`   | Interval  | Interval      |
| any | <code>&#x7c;&#x7c;</code> | any | String |
| Number    | `&`   | Number    | Number        |
| Number | <code>&#x7c;</code> | Number | Number |
| Number    | `^`   | Number    | Number        |
| any       | `=`   | any       | Number        |
| any       | `<>`  | any       | Number        |
| any       | `<`   | any       | Number        |
| any       | `>`   | any       | Number        |
| any       | `<=`  | any       | Number        |
| any       | `>=`  | any       | Number        |
| any       | `IS`  | any       | Number        |
| any   | `IS NOT`  | any       | Number        |
|           | `NOT` | Number    | Number        |
| Number    | `AND` | Number    | Number        |
| Number    | `OR`  | Number    | Number        |
| variable  | `:=`  | any       | any           |
| any       | `;`   | any       | any           |

* **Division `/`**

    The division operator always result in a floating-point number (i.e. `3 / 2 = 1.5`). Use the
    `div` function for integer division.

* **Concatenation `||`**

    The `||` operator concatenates two strings together. If either side is not a string, they will
    first be converted into a string, except that NULLs are propagated (`NULL||x` produces `NULL`).
    This operator cannot be used to concatenate arrays.

* **Comparison `=`, `<>`, `<`, `>`, `<=`, `>=`**

    These operators will return TRUE, FALSE or NULL. When comparing two values, `dbgen` follows
    these rules:

    - Comparing with NULL always return NULL.
    - Numbers, timestamps and intervals are ordered by values.
    - Strings are ordered lexicographically in the UTF-8 binary collation.
    - Arrays are ordered lexicographically by their elements.
    - Comparing two values with different types (e.g. `'4' < 5`) will abort the program.

* **Identity `IS`, `IS NOT`**

    These operators will return TRUE or FALSE. `dbgen` follows these rules:

    - `NULL IS NULL` is TRUE.
    - Values having different types are not identical (`'4' IS 5` is FALSE).
    - Values having the same types compare like the `=` and `<>` operators.

    These operators are a generalization of standard SQL's `IS [NOT] {TRUE|FALSE|NULL}` operators.

* **Logical operators `NOT`, `AND`, `OR`**

    These operators will first convert the input into a nullable boolean value
    (TRUE, FALSE or NULL):

    - NULL remains NULL.
    - Nonzero numbers become TRUE, `0` and `0.0` becomes FALSE, and `NaN` becomes NULL.
    - All other types cannot be converted to a boolean and will abort the program.

    The trinary logic operates like this:

    |       AND |  TRUE |  NULL | FALSE |
    |----------:|:-----:|:-----:|:-----:|
    |  **TRUE** |  TRUE |  NULL | FALSE |
    |  **NULL** |  NULL |  NULL | FALSE |
    | **FALSE** | FALSE | FALSE | FALSE |

    |    OR     |  TRUE |  NULL | FALSE |
    |----------:|:-----:|:-----:|:-----:|
    |  **TRUE** |  TRUE |  TRUE |  TRUE |
    |  **NULL** |  TRUE |  NULL |  NULL |
    | **FALSE** |  TRUE |  NULL | FALSE |

    |   NOT     | value |
    |----------:|:-----:|
    |  **TRUE** | FALSE |
    |  **NULL** |  NULL |
    | **FALSE** |  TRUE |

* **Bitwise operators `&`, `|`, `^`, `~`**

    These corresponds to bitwise-AND, -OR, -XOR and -NOT respectively. These
    operators only accept integers as input and produce *signed* results (e.g.
    `~1 = -2`).

* **Assignment `:=`**

    The assignment expression `@ident := f()` would evaluate the RHS `f()` and save into the local
    variable `@local`. The same value can later be extracted using `@local`. This can be used to
    generate correlated columns, for instance:

    ```sql
    CREATE TABLE _ (
        "first"  BOOLEAN NOT NULL {{ rand.bool(0.5) }},
        "second" BOOLEAN NOT NULL {{ @a := rand.bool(0.5) }},
        "third"  BOOLEAN NOT NULL {{ @a }}
    );
    ```

    The first and second columns are entirely independent, but the second and third column will
    always have the same value.

    Within the same generated file, a row can use local variables assigned from the previous row.
    For instance:

    ```sql
    {{ @prev := 0 }}
    CREATE TABLE _ (
        "prev" INTEGER NULL     {{ @prev }},
        "cur"  INTEGER NOT NULL {{ @prev := rownum }}
    );
    ```

    would produce

    ```sql
    INSERT INTO _ VALUES
    (0, 1),
    (1, 2),
    (2, 3),
    …
    ```

* **Statements `;`**

    The syntax `a; b; c` evaluates all 3 expressions in order, but only returns `c`. The results of
    `a` and `b` are discarded. The statement separator `;` can only be used

    * directly inside `{{ … }}`, or
    * as `THEN`/`ELSE` clauses of `CASE WHEN` expressions

### Symbols

* **rownum**: The current row number of the main table. The first row has value 1. The derived rows share the same row
    number as the main row.
* **subrownum**: The current number in a derived table. If one row of the main table generates *N* rows in the derived
    table, this constant will take values 1, 2, …, *N*.
* **current_timestamp**: The timestamp when `dbgen` was started. This can be overridden using the `--now` parameter.
* **NULL**: The null value.
* **TRUE**: The true value.
* **FALSE**: The false value.

### Random functions

* **rand.regex('[0-9a-z\-]+', 'i', 100)**

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

* **rand.bool(0.3)**

    Generates a random boolean (FALSE or TRUE) with probability 0.3 of getting TRUE. Also known as
    the Bernoulli distribution.

* **rand.zipf(26, 0.8)**

    Generates a random integer in the closed interval 1 ≤ *x* ≤ 26 using [Zipfian distribution]
    with an exponent of 0.8.

    With Zipfian distribution, the smallest values will appear more often.

    [Zipfian distribution]: https://en.wikipedia.org/wiki/Zipf's_law

* **rand.log_normal(2.0, 3.0)**

    Generates a random positive number using the [log-normal distribution]
    (log *N*(*µ*, *σ*<sup>2</sup>)) with *μ* = 2.0 and *σ* = 3.0.

    The median of this distribution is exp(*µ*).

    [log-normal distribution]: https://en.wikipedia.org/wiki/Log-normal_distribution

* **rand.finite_f32()**, **rand.finite_f64()**

    Generates a random finite IEEE-754 binary32 or binary64 floating-point number.

    The numbers are uniform in its *bit-pattern* across the entire supported range
    (±3.4 × 10<sup>38</sup> for `f32`, ±1.8 × 10<sup>308</sup> for `f64`)

* **rand.uuid()**

    Generates a [version 4 (random) UUID](https://tools.ietf.org/html/rfc4122#section-4.4).

    The result is a string in the format `'aaaaaaaa-bbbb-4ccc-9ddd-eeeeeeeeeeee'`.

### Date and Time

* **TIMESTAMP '2016-01-02 15:04:05.999'**

    Converts an ISO-8601-formatted string into a timestamp.

    The timestamp is always measured in UTC time zone.

* **INTERVAL 30 MINUTE**

    Creates a time interval. The inner expression should evaluate a number (can be negative). Valid
    units are:

    - MICROSECOND
    - MILLISECOND
    - SECOND
    - MINUTE
    - HOUR
    - DAY
    - WEEK

    Intervals can be added to or subtracted from timestamps, and can therefore be used to generate
    a random timestamp.

* **rand.u31_timestamp()**

    Generates a random timestamp distributed uniformly between 1970-01-01 00:00:01 and
    2038-01-19 03:14:07 (UTC), inclusively. There are exactly 2<sup>31</sup>−1 seconds between these
    two time.

### Strings

* **substring('ⓘⓝⓟⓤⓣ' FROM 2 FOR 3 USING CHARACTERS)**

    Extracts a substring from character 2 with length of 3 characters. "Character" means a Unicode
    codepoint here. Following SQL standard, the character position is 1-based, so this function call
    returns `'ⓝⓟⓤ'`.

    All of `FROM`, `FOR` and `USING` parts are optional. The `FROM` part defaults to 1 (start of
    string), and `FOR` part defaults to length of the string, e.g.

    ```sql
    substring('ⓘⓝⓟⓤⓣ' FOR 3) = 'ⓘⓝⓟ';
    substring('ⓘⓝⓟⓤⓣ' FROM 3) = 'ⓟⓤⓣ';
    ```

* **substring('input' FROM 2 FOR 3 USING OCTETS)**

    Extracts a substring from byte 2 with length of 3 bytes. Following SQL standard, the byte
    position is 1-based, so this function call returns `'npu'`.

    Both the `FROM` and `FOR` parts are optional. The `FROM` part defaults to 1 (start of string),
    and `FOR` part defaults to length of the string.

* **octet_length('input')**

    Computes the byte length of the input string.

* **char_length('ⓘⓝⓟⓤⓣ')**, **character_length('ⓘⓝⓟⓤⓣ')**

    Computes the character length of the input string. "Character" means a Unicode codepoint here.
    `character_length` is an alias of `char_length`; the two functions are equivalent.

* **overlay('input' PLACING 'replacement' FROM 2 FOR 3 USING CHARACTERS)**

    Replaces the substring of `'input'` by the `'replacement'`. The meaning of `FROM`, `FOR` and
    `USING` when specified are equivalent to the `substring()` function.

    The `FOR` and `USING` parts are optional. The `FOR` part defaults to the length of the
    replacement string.

### Encodings

* **to_hex('¿?')**

    Encodes a byte string into a hexadecimal (Base16) string.

    The result will be in uppercase (e.g. `'C2BF3F'`).

* **to_base64('¿¡input!?')**

    Encodes a byte string with Base64 encoding (RFC 4648).

    The result uses the alphabet `[A-Za-z0-9+/]` and the padding character `=`. No new lines will be
    inserted.

* **to_base64url('¿¡input!?')**

    Encodes a byte string with Base64URL encoding (RFC 4648).

    The result uses the alphabet `[A-Za-z0-9\-_]` without paddings.

* **from_hex('c2bf 3f')**, **X'C2BF 3F'**

    Decodes a hexadecimal (Base16) string.

    The string content must be an even number of hex digits (`[0-9A-Fa-f]`), but ASCII spaces will
    be ignored when decoding. The program stops when there is an invalid digit or the number of
    digits is odd.

    The `X'…'` form is an alias of the `from_hex` function; these two forms are equivalent.

* **from_base64('wr/CoWlucHV0IT8=')**, **from_base64url('wr_CoWlucHV0IT8')**

    Decodes a Base64 or Base64URL encoded string (RFC 4648).

    The string content can only include the Base64 (`[A-Za-z0-9+/]`) or Base64URL alphabet
    (`[A-Za-z0-9\-_]`), but ASCII spaces and the padding `=` will be ignored when decoding.
    The program stops when there is an invalid character.

    The `from_base64` and `from_base64url` functions are synonyms.

### Numbers

* **greatest(*x*, *y*, *z*)**

    Returns the largest of all given values. NULL values are ignored.

* **least(*x*, *y*, *z*)**

    Returns the smallest of all given values. NULL values are ignored.

* **round(456.789, 2)**

    Rounds the number 456.789 to 2 decimal places (i.e. returns 456.79).

    The decimal place argument is optional, and defaults to 0. It can also be negative to round by
    powers of 10, e.g. `round(456.789, -2) = 500.0`. In case of break-even (e.g. `round(3.5)`), this
    function will round half away from zero.

* **div(9, 4)**, **mod(9, 4)**

    Computes the quotient and remainder respectively when 9 is divided by 4 (i.e. 2). These two
    functions are related by

    ```sql
    n = div(n, d)*d + mod(n, d)
    ```

    The `div(n, d)` function is equivalent to `n / d` truncated towards 0.

    The result of `mod(n, d)` has the same sign as the numerator `n`.

    When the denominator `d` is 0, both of these functions return NULL.

    `n` and `d` may be both numbers or both time intervals.

### Arrays

* **ARRAY['X', 'Y', 'Z']**

    Constructs an array with content 'X', 'Y', 'Z'.

* ***arr*[3]**

    Extracts the 3rd element from the array *arr*. Following the SQL standard, the index is 1-based,
    i.e. *arr*[1] returns the first element. Returns NULL if the index is out of range of the array.

* **generate_series(11, 31, 5)**

    Generates an array of value sequence `array[11, 16, 21, 26, 31]`. Both start and end points are
    inclusive.

    The step ("5" here) can be omitted and defaults to 1. It can also be negative to generate a
    decreasing sequence.

    ```sql
    generate_series(31, 11, -5) = array[31, 26, 21, 16, 11]
    ```

    The sequence will not go beyond the end point.

    ```sql
    generate_series(11, 30, 5) = array[11, 16, 21, 26]
    generate_series(30, 11, -5) = array[30, 25, 20, 15]
    ```

    The elements produced by `generate_series()` are lazily computed with constant memory usage.

* **rand.shuffle(*arr*)**

    Returns a new array by shuffling *arr*.

    When the array length is very long, this function switches from the standard Fisher-Yates
    shuffle to an FPE-inspired algorithm (currently 8-round Feistel network) to ensure memory usage
    remains constant. Together with `generate_series()` one could produce a sequence of values that
    are guaranteed to be distinct but at the same time randomly distributed.

    ```sql
    /*{{ @phone_numbers := rand.shuffle(generate_series(2000000000, 9999999999)) }}*/
    CREATE TABLE accounts (
        id BIGINT PRIMARY KEY /*{{ rownum }}*/,
        phone_number VARCHAR(10) /*{{ @phone_numbers[rownum] }}*/
    );
    ```

### Debugging

* **debug.panic('X', 'Y', 'Z')**

    Causes the generation to fail with error when this function is executed. The input to
    `debug.panic()` will be evaluated and printed out, in the form like:

    ```
    Error: runtime panic:
     1. X
     2. Y
     3. Z
      --> 99:3
       |
    99 | {{ debug.panic('X', 'Y', 'Z') }}␊
       |    ^-------------------------^
       |
       =
    ```

### Miscellaneous

* **CASE *value* WHEN *p1* THEN *r1* WHEN *p2* THEN *r2* ELSE *ro* END**

    Equivalent to the SQL simple `CASE … WHEN` expression.

    If *value* equals to *p1* (i.e. `(value = p1) IS TRUE`), then the expression's value is *r1*,
    etc. If the *value* does not equal to any of the listed pattern, the value *ro* will be
    returned. If the ELSE branch is missing, returns NULL.

* **CASE WHEN *p1* THEN *r1* WHEN *p2* THEN *r2* ELSE *ro* END**

    Equivalent to the SQL searched `CASE WHEN` expression.

    If *p1* is true, then the expression's value is *r1*, etc. If all of the listed conditions are
    false or NULL, the value *ro* will be returned. If the ELSE branch is missing, returns NULL.

* **coalesce(*v1*, *v2*, *v3*)**

    Returns the first non-NULL value. If all of *v1*, *v2*, *v3* are NULL, returns NULL.

    Note that `coalesce` is treated as a normal function, unlike standard SQL, and all arguments are
    evaluated before checking for nullability. Prefer `CASE WHEN` expression if you need to control
    the evaluation side-effect.

* **@local**

    Gets the previous assigned local variable. If the variable was undefined, this will return NULL.
