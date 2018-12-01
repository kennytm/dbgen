Schema generator CLI usage
==========================

```sh
dbschemagen -d mysql -s test_db -z 1e9 -t 5 -- --escape-backslash > gen.sh
sh gen.sh
```

Common options
--------------

* `-d «DIALECT»`, `--dialect «DIALECT»`

    Choose the SQL dialect of the generated schema files. This mainly controls the data type names.

* `-s «NAME»`, `--schema-name «NAME»`

    The qualified schema name.

* `-z «SIZE»`, `--size «SIZE»`

    The estimated total size of the generated data file.

* `-t «N»`, `--tables-count «N»`

    Number of tables to generate.

    Note that `dbschemagen` will *not* uniformly distribute the same size to every file; rather,
    they're assigned following to Pareto distribution to simulate the size of real-world databases.

* `-- «args»...`

    Any extra arguments will be passed to the `dbgen` invocations.


More options
------------

* `-n «N»`, `--inserts-count «N»`

    Number of INSERT statements per file.

* `-r «N»`, `--rows-count «N»`

    Number of rows per INSERT statement.

