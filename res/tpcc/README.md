TPC-C-compatible templates for `dbgen`
======================================

This folder provides template files and a Python script to produce SQL dump compatible with
the [TPC-C] v5.11.0 benchmark. The table names are compatible with [BenchmarkSQL].

|                          | `dbgen`                           | BenchmarkSQL      |
|--------------------------|----------------------------------:|------------------:|
| Output format            | SQL dump split into 256 MiB files | 8 large CSV files |
| Total size per warehouse | 80 MiB                            | 70 MiB            |
| Speed (-j 8, W = 30)     | 30s                               | 50s               |
| Speed (-j 8, W = 50)     | 40s                               | 80s               |

## Usage

1. Download or build `dbdbgen`.

    Pre-compiled binaries can be downloaded from <https://github.com/kennytm/dbgen/releases>.
    Decompress the `*.tar.xz` from the assets of the latest release to get the `dbdbgen` executable.

    You can also build `dbdbgen` from source with Rust 1.40 (or above). After installing Rust, run
    `cargo build --release -p dbdbgen`.

2. Execute the `dbdbgen` program. Suppose we want to create a 30-warehouse dump in the `tpcc-out/`
    folder:

    ```sh
    dbdbgen res/tpcc/tpcc.jsonnet -o tpcc-out -w 30
    ```

    <details><summary>The SQL dump is split into multiple files in subdirectories of
    <code>tpcc-out</code>. They are lexicographically sorted by the proper import order.</summary>

    ```
    tpcc-out/
        0_config/
            tpcc-schema-create.sql
            tpcc.bmsql_config-schema.sql
            tpcc.bmsql_config.1.sql
        1_item/
            tpcc.bmsql_item-schema.sql
            tpcc.bmsql_item.1.sql
        2_warehouse/
            tpcc.bmsql_warehouse-schema.sql
            tpcc.bmsql_warehouse.1.sql
        3_stock/
            tpcc.bmsql_stock-schema.sql
            tpcc.bmsql_stock.001.sql
            tpcc.bmsql_stock.002.sql
            …
        4_district/
            tpcc.bmsql_district-schema.sql
            tpcc.bmsql_district.1.sql
        5_customer/
            tpcc.bmsql_customer-schema.sql
            tpcc.bmsql_customer.01.sql
            tpcc.bmsql_customer.02.sql
            …
        6_history/
            tpcc.bmsql_history-schema.sql
            tpcc.bmsql_history.01.sql
            tpcc.bmsql_history.12.sql
            …
        7_order/
            tpcc.bmsql_oorder-schema.sql
            tpcc.bmsql_oorder.001.sql
            tpcc.bmsql_oorder.002.sql
            …
            tpcc.bmsql_order_line-schema.sql
            tpcc.bmsql_order_line.001.sql
            tpcc.bmsql_order_line.002.sql
            …
        8_new_order/
            tpcc.bmsql_new_order-schema.sql
            tpcc.bmsql_new_order.1.sql
            tpcc.bmsql_new_order.2.sql
            …
    ```

    </details>

5. Load the SQL dump into the database. Typically you can simply pipe the files into the database
    client, e.g.

    * **SQLite3**

        ```sh
        export LANG=C
        rm -f tpcc.db
        for f in tpcc-out/*/*.*.sql; do
            echo "$f"
            sqlite3 tpcc.db < "$f" || break
        done
        ```

    * **PostgreSQL via `psql`**

        Make sure you have CREATE privilege in the chosen database to create the `tpcc` schema.

        ```sh
        export LANG=C                        # make sure '-' is sorted before '.'
        export PGOPTIONS=--search_path=tpcc
        psql postgres -c 'drop schema if exists tpcc cascade;'
        for f in tpcc-out/*/*.sql; do
            echo "$f"
            psql postgres -q -1 -v ON_ERROR_STOP=1 -f "$f" || break
        done
        ```

    * **MySQL via `mysql`**

        Make sure you have CREATE privilege to create the `tpcc` database.

        ```sh
        export LANG=C
        mysql -u root 'drop schema if exists tpcc; create schema tpcc;'
        for f in tpcc-out/*/*.sql; do
            echo "$f"
            mysql -u root < "$f" || break
        done
        ```

    * **MySQL via [myloader]**

        `myloader` restores an SQL dump into MySQL in parallel. It automatically manages the import
        order but expects SQL files in a flat directory, so we first need to flatten it. Then we can
        ingest the entire directory in one go.

        ```sh
        # Transform the output directory into mydumper structure.
        mv tpcc-out/*/* tpcc-out/
        touch tpcc-out/metadata
        # Disable foreign key checks, since the files are imported in no particular order.
        mysql -u root -e 'set @@global.foreign_key_checks = 0;'
        # Now import the entire directory.
        myloader -u root -B tpcc -d tpcc-out/
        # Re-enable foreign key checks.
        mysql -u root -e 'set @@global.foreign_key_checks = 1;'
        ```

    * **[TiDB] via [TiDB Lightning]**

        The output structure is directly compatible with TiDB Lightning and can be used directly.

        Note that, before v4.0, TiDB does not support the SERIAL alias. You may need to manually
        replace its use in `6_history/tpcc.bmsql_history-schema.sql` as
        BIGINT UNSIGNED AUTO_INCREMENT first.

        ```sh
        sed -i'' 's/serial/bigint unsigned auto_increment/' tpcc-out/6_history/tpcc.bmsql_history-schema.sql
        # ^ Not needed for TiDB Lightning 4.0 or above
        tidb-lightning -d tpcc-out/ --tidb-host 127.0.0.1
        ```

[TPC-C]: http://www.tpc.org/tpcc/
[BenchmarkSQL]: https://sourceforge.net/projects/benchmarksql/
[myloader]: https://github.com/maxbube/mydumper
[TiDB]: https://pingcap.com/docs/
[TiDB Lightning]: https://pingcap.com/docs/stable/reference/tools/tidb-lightning/overview/