Template reference
==================

File syntax
-----------

The template file should consist of one CREATE TABLE statement, with expressions telling how a value
should be generated inside `{{ … }}` or `/*{{ … }}*/` blocks:

```sql
CREATE TABLE "database"."schema"."table" (
    column_1    COLUMN_TYPE_1,
        {{ value_1 }}
    column_2    COLUMN_TYPE_2,
        /*{{ value_2 }}*/
    -- ...
    column_n    COLUMN_TYPE_N,
        {{ value_n }}
    INDEX(some_index),
    INDEX(more_index)
) OPTION_1 = 1 /*, ... */;
```

Expression syntax
-----------------

`dbgen` supports an SQL-like expression syntax. These expressions will be re-evaluated for every new
row generated.

### Literals

`dbgen` supports numbers and string literals.

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

1. unary `-`, unary `+`, function call
2. `*`, `/`
3. `+`, `-`, `||`
4. `=`, `<>`, `<`, `>`, `<=`, `>=`, `IS`, `IS NOT`
5. unary `NOT`
6. `AND`
7. `OR`
8. `:=`

* **Concatenation `||`**

    The `||` will concatenate two strings together. If either side is not a string, they will first
    be converted into a string.

* **Comparison `=`, `<>`, `<`, `>`, `<=`, `>=`**

    These operators will return TRUE, FALSE or NULL. When comparing two values, `dbgen` follows
    these rules:

    - Comparing with NULL always return NULL.
    - Numbers are ordered by values.
    - Strings are ordered lexicographically in the UTF-8 binary collation.
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

### Symbols

* **rownum**: The current row number. The first row has value 1.
* **NULL**: The null value.
* **TRUE**: Equals to 1.
* **FALSE**: Equals to 0.

### Random functions

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

* **rand.bool(0.3)**

    Generates a random boolean (0 or 1) with probability 0.3 of getting "1". Also known as the
    Bernoulli distribution.

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

### Date and Time

* **TIMESTAMP '2016-01-02 15:04:05.999'**

    Converts an ISO-8601-formatted string into a timestamp, using the time zone specified by the
    `--time-zone` flag. The timestamp is internally stored as UTC.

    If a time zone observes DST, there will be some time values which are impossible or ambiguous.
    Both of these cases will cause an "invalid timestamp" error.

* **TIMESTAMP WITH TIME ZONE '2016-01-02 15:04:05.999 Asia/Hong_Kong'**

    Converts an ISO-8601-formatted string into a timestamp, using the time zone specified inside
    the string. The timestamp is internally stored as UTC.

    Only names in the `tz` database are recognized. The time zone will **not** be printed together
    with the timestamp.

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
    2038-01-19 03:14:07 (UTC). There are exactly 2<sup>31</sup>−1 seconds between these two time.

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

* **@local**

    Gets the previous assigned local variable. If the variable was undefined, this will return NULL.

* **greatest(*x*, *y*, *z*)**

    Returns the largest of all given values. NULL values are ignored.

* **least(*x*, *y*, *z*)**

    Returns the smallest of all given values. NULL values are ignored.

* **round(456.789, 2)**

    Rounds the number 456.789 to 2 decimal places (i.e. returns 456.79).

    The decimal place argument is optional, and defaults to 0. It can also be negative to round by
    powers of 10, e.g. `round(456.789, -2) = 500.0`. In case of break-even (e.g. `round(3.5)`), this
    function will round half away from zero.
