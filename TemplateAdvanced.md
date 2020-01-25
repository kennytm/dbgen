Advanced template features
==========================

## Global expressions

The `{{ … }}` blocks can be placed before the first CREATE TABLE statement. These expressions would
be evaluated once, and will not be written into the generated files. This is useful to define global
constants used by all rows.

```sql
{{ @dirs := array['North', 'West', 'East', 'South'] }}
CREATE TABLE cardinals (
    t INTEGER       {{ rownum }},
    d1 VARCHAR(5)   {{ @dirs[rand.zipf(4, 0.8)] }},
    d2 VARCHAR(5)   {{ @dirs[rand.zipf(4, 0.8)] }}
);
```

Variables assigned in global expressions can be re-assigned, but the change is localized in the
current generated file. Every new file would be initialized by the same evaluated values.
For instance if we generate 2 files given this template:

```sql
{{ @value := rand.range(0, 100000) }}
CREATE TABLE _ (
    p INTEGER {{ @value }},
    n INTEGER {{ @value := rand.range(0, 100000) }}
);
```

We may get

```sql
------ first file -------
INSERT INTO _ VALUES
(58405, 87322),
(87322, 41735),
(41735, 91701);

------ second file ------
INSERT INTO _ VALUES
(58405, 3046),
(3046, 8087),
(8087, 26211);
```

Note that the initial `@value` are the same for both files (`58405`), because `rand.range()` is only
evaluated once. After generation started, though, each file acquires its own state and we see they
evaluate `@value` differently without any interference.

## Derived tables

In a relational database, contents of tables are related to each other, e.g.

```sql
CREATE TABLE "parent" (
    "parent_id"     UUID PRIMARY KEY,
    "child_count"   INT UNSIGNED NOT NULL
);

CREATE TABLE "child" (
    "child_id" UUID PRIMARY KEY,
    "parent_id" UUID NOT NULL REFERENCES "parent"("parent_id")
);
```

We want the two tables to be related such that:

* `child.parent_id` refer to real IDs in the `parent` table
* `parent.child_count` is an actual count of rows in `child` table having the specified `parent_id`.
* `parent.child_count` are still random.

These two tables therefore must be generated together. `dbgen` supports generating *derived tables*
from the previous tables with this syntax:

```sql
CREATE TABLE "parent" (
    "parent_id" UUID PRIMARY KEY,
        /*{{ @parent_id := rand.uuid() }}*/
    "child_count" INT UNSIGNED NOT NULL
        /*{{ @child_count := rand.range_inclusive(0, 4) }}*/
);

/*{{ for each row of "parent" generate @child_count rows of "child" }}*/
CREATE TABLE "child" (
    "child_id" UUID PRIMARY KEY,
        /*{{ rand.uuid() }}*/
    "parent_id" UUID NOT NULL REFERENCES "parent"("parent_id")
        /*{{ @parent_id }}*/
);
```

This may produce

```sql
------ parent.1.sql ------
INSERT INTO "parent" VALUES
('451b789a-3438-4d6b-847e-ac6bb0d61988', 0),
('55200ffe-2304-4b68-a1a8-8467fbcbb339', 4),
('0082fa2d-c553-46df-aa61-7182accf1ea7', 2),
('c488c641-a92e-405c-870b-1e10a213e456', 1),
…

------ child.1.sql -------
INSERT INTO "child" VALUES
('49188e47-d0da-4f1e-8c82-156138bb4887', '55200ffe-2304-4b68-a1a8-8467fbcbb339'),
('0251ec50-8039-4e59-a04f-fc8143a9d278', '55200ffe-2304-4b68-a1a8-8467fbcbb339'),
('4dddc583-b175-4814-a677-02fa4ec295b8', '55200ffe-2304-4b68-a1a8-8467fbcbb339'),
('fb8bab0d-8f3a-4cf8-891d-d2ad6e7aac28', '55200ffe-2304-4b68-a1a8-8467fbcbb339'),
('1feb2f81-6000-4191-8cc3-95acbd3f1723', '0082fa2d-c553-46df-aa61-7182accf1ea7'),
('63e44b85-1779-4508-9598-c94df3eee10e', '0082fa2d-c553-46df-aa61-7182accf1ea7'),
('77d13d62-12ea-4fe7-98c5-35cb0f1daece', 'c488c641-a92e-405c-870b-1e10a213e456'),
…
```

There can be multiple derived tables, and it can refer to any table before it as the generator.

```sql
CREATE TABLE A ( … );
/*{{ for each row of A generate 2 rows of B }}*/
CREATE TABLE B ( … );
/*{{ for each row of B generate 1 row of C }}*/
CREATE TABLE C ( … );
/*{{ for each row of A generate 4 rows of D }}*/
CREATE TABLE D ( … );
```

All derived rows share the same set of variables. Variables can be used to establish common values
among the group of tables.

### `rownum` and `subrownum`

In a derived table, `rownum` refers to the row number of the *main* table. If we generate 10 derived
rows for each main row, all 10 rows will produce the same `rownum`.

You can distinguish between derived rows of the same `rownum` using the `subrownum` symbol, which
has values 1, 2, …, 10 if we generate 10 rows.

```sql
-- INPUT: template.sql
CREATE TABLE main ( … );
/*{{ for each row of main generate 3 rows of derived }}*/
CREATE TABLE derived (
    rn INT /*{{ rownum }}*/,
    srn INT /*{{ subrownum }}*/,
    …
);

-- RESULT: derived.1.sql
INSERT INTO derived VALUES
(1, 1, …),
(1, 2, …),
(1, 3, …),
(2, 1, …),
(2, 2, …),
(2, 3, …),
…
```

With a derived table hierarchy, the `rownum` always refer to the top table, and `subrownum` always
refer to the current table. If you need the row numbers of the tables in between, store them into a
variable, e.g.

```sql
-- INPUT: template.sql
CREATE TABLE "top" ( top_id INT /*{{ rownum }}*/, … );
/*{{ for each row of "top" generate 2 rows of "middle" }}*/
CREATE TABLE "middle" ( middle_id INT /*{{ @middle_id := subrownum }}*/, … );
/*{{ for each row of "middle" generate 2 rows of "bottom" }}*/
CREATE TABLE "bottom" (
    top_id INT /*{{ rownum }}*/,
    middle_id INT /*{{ @middle_id }}*/,
    bottom_id INT /*{{ subrownum }}*/,
    …
);
```

### File size concern

Derived tables do not have individual `--files-count`, `--inserts-count` and `--rows-count`
settings. In particular, if we set `for each row of "main" generate N rows of "derived"`, the actual
number of rows per INSERT statements of the derived table will be N times `--row-count` of the main
table (deeper derivatives will cascade). This may produce excessively large tables when the number
of rows to generate is huge. Therefore, if it is possible to generate the values independently, we
recommend using two separate templates instead of derived tables.

<table><tr>
<td>NOT recommended. File size of main and derived data cannot be balanced.</td>
<td>Recommended. File size of main and derived data can be balanced.</td>
</tr>
<tr><td>

```sql
-- template.sql
CREATE TABLE main (
    main_id INT PRIMARY KEY {{ rownum }}
);
{{ for each row of main generate 3000 rows of derived }}
CREATE TABLE derived (
    main_id INT NOT NULL {{ rownum }},
    sub_id  INT NOT NULL {{ subrownum }},
    PRIMARY KEY (main_id, sub_id)
);
```

</td><td>

```sql
-- main.sql
CREATE TABLE main (
    main_id INT PRIMARY KEY {{ rownum }}
);
```

```sql
-- derived.sql
CREATE TABLE derived (
    main_id INT NOT NULL {{ div(rownum-1, 3000)+1 }},
    sub_id  INT NOT NULL {{ mod(rownum-1, 3000)+1 }},
    PRIMARY KEY (main_id, sub_id)
);
```

</td></tr></table>