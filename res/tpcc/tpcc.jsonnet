local dbdbgen = import 'dbdbgen.libsonnet';
{
    name: 'tpcc.jsonnet',
    version: '0.8.0',
    about: 'Generate TPC-C-compatible *.sql dump for MySQL and PostgreSQL',

    args: dbdbgen.stdArgs {
        warehouses: {
            short: 'w',
            help: 'Number of warehouses.',
            type: 'int',
            required: true,
        },
        nurand_c: {
            long: 'nurand-c',
            help: 'Constant "C" used in NUrand() function for C_LAST column.',
            type: 'int',
        },
        table_prefix: {
            short: 't',
            long: 'table-prefix',
            help: 'Table name prefix, must not be quoted.',
            default: 'bmsql_',
        },
        schema_name+: {
            default: 'tpcc',
        },
        foreign_key: {
            long: 'foreign-key',
            help: 'Enable foreign keys in the generated schema.',
            type: 'bool',
        },

        // TPC-C output contains no special characters.
        escape_backslash:: null,
        // TPC-C output contains no timestamps.
        now:: null,
    },

    steps(m)::
        // The total file size needed is roughly (80.3*W + 8.3) MiB
        local format = {
            nurand_c: if 'nurand_c' in m then m.nurand_c else std.parseHex(m.seed[:2]),
            prefix: m.table_prefix,
            warehouses: m.warehouses,
            fk: if m.foreign_key then '' else '-- ',
        };
        [
            /* 0_config
            fixed rows for bmsql compatibility */
            m {
                out_dir+: '/0_config',
                seed: dbdbgen.xorSeed(m.seed, '4beb30572405220384a4546d8af31cb8e34242e738d0135d82676d13df059e94'),
                total_count: 4,
                rows_per_file: 4,
                components+: ['schema'],
                template_string: |||
                    /*{{
                        @names := array['warehouses', 'nURandCLast', 'nURandCC_ID', 'nURandCI_ID'];
                        @values := array[%(warehouses)d, %(nurand_c)d, rand.range(0, 1024), rand.range(0, 8192)]
                    }}*/
                    create table %(prefix)sconfig (
                        cfg_name    varchar(30),
                            /*{{ @names[rownum] }}*/
                        cfg_value   varchar(50),
                            /*{{ @values[rownum] || '' }}*/
                        primary key (cfg_name)
                    );
                ||| % format,
            },

            // The remaining schemas are configured according to TPC-C v5.11 §4.3.3.1.

            /* 1_item

                100,000 rows in the ITEM table with:
                - I_ID     unique within [100,000]
                - I_IM_ID  random within [1 .. 10,000]
                - I_NAME   random a-string [14 .. 24]
                - I_PRICE  random within [1.00 .. 100.00]
                - I_DATA   random a-string [26 .. 50]. For 10% of the rows, selected at random, the
                        string "ORIGINAL" must be held by 8 consecutive characters starting at
                        random position within I_DATA

            fixed 100,000 rows, ~8.3 MiB */
            m {
                out_dir+: '/1_item',
                seed: dbdbgen.xorSeed(m.seed, '9bbc4e1d35eae5a57b67ff3535d78523b750c67ad001e61e83babb82cb2c2295'),
                total_count: 100e3,
                rows_per_file: 100e3,
                template_string: |||
                    create table %(prefix)sitem (
                        i_id    integer,
                            /*{{ rownum }}*/
                        i_name  varchar(24),
                            /*{{ rand.regex('[0-9a-zA-Z]{14,24}') }}*/
                        i_price decimal(5,2),
                            /*{{ rand.range_inclusive(100, 10000)/100 }}*/
                        i_data  varchar(50),
                            /*{{
                                overlay(
                                    (@data := rand.regex('[0-9a-zA-Z]{26,50}'))
                                    placing
                                    (case when rand.bool(0.1) then 'ORIGINAL' else '' end)
                                    from
                                    (rand.range(1, octet_length(@data) - 7))
                                    using octets
                                )
                            }}*/
                        i_im_id integer,
                            /*{{ rand.range_inclusive(1, 10000) }}*/
                        primary key (i_id)
                    );
                ||| % format,
            },

            /* 2_warehouse

                1 row in the WAREHOUSE table for each configured warehouse with:
                - W_ID       unique within [number_of_configured_warehouses]
                - W_NAME     random a-string [6 .. 10]
                - W_STREET_n random a-string [10 .. 20]
                - W_CITY     random a-string [10 .. 20]
                - W_STATE    random a-string of 2 letters
                - W_ZIP      generated according to Clause 4.3.2.7
                - W_TAX      random within [0.0000 .. 0.2000]
                - W_YTD    = 300,000.00

            W rows, 113 B/warehouse => 2,375,535 warehouses/file */
            m {
                out_dir+: '/2_warehouse',
                seed: dbdbgen.xorSeed(m.seed, '86249c82e1e853117f865c69a4756b644218b3afa4e43d6e7d2e538fe31ddfdd'),
                total_count: m.warehouses,
                rows_per_file: 2.4e6,
                template_string: |||
                    create table %(prefix)swarehouse (
                        w_id        integer not null,
                            /*{{ rownum }}*/
                        w_ytd       decimal(12,2),
                            /*{{ 300000.0 }}*/
                        w_tax       decimal(4,4),
                            /*{{ rand.range_inclusive(0, 2000)/10000 }}*/
                        w_name      varchar(10),
                            /*{{ rand.regex('[0-9a-zA-Z]{6,10}') }}*/
                        w_street_1  varchar(20),
                            /*{{ rand.regex('[0-9a-zA-Z]{10,20}') }}*/
                        w_street_2  varchar(20),
                            /*{{ rand.regex('[0-9a-zA-Z]{10,20}') }}*/
                        w_city      varchar(20),
                            /*{{ rand.regex('[0-9a-zA-Z]{10,20}') }}*/
                        w_state     char(2),
                            /*{{ rand.regex('[A-Z]{2}') }}*/
                        w_zip       char(9),
                            /*{{ rand.regex('[0-9]{4}11111') }}*/
                        primary key (w_id)
                    );
                ||| % format,
            },

            /* 3_stock

                100,000 rows in the STOCK table with:
                - S_I_ID         unique within [100,000]
                - S_W_ID       = W_ID
                - S_QUANTITY     random within [10 .. 100]
                - S_DIST_nn      random a-string of 24 letters
                - S_YTD        = 0
                - S_ORDER_CNT  = 0
                - S_REMOTE_CNT = 0
                - S_DATA         random a-string [26 .. 50]. For 10% of the rows, selected at
                                 random, the string "ORIGINAL" must be held by 8 consecutive
                                 characters starting at random position within S_DATA

            100,000*W rows, 33 MiB/warehouse => 8 warehouses/file */
            m {
                out_dir+: '/3_stock',
                seed: dbdbgen.xorSeed(m.seed, '6243f087846cddc91aa76af97543661bfd668c6d47841b66b7b71c86efbd53c3'),
                total_count: 100e3 * m.warehouses,
                rows_per_file: 800e3,
                template_string: |||
                    create table %(prefix)sstock (
                        s_w_id       integer not null,
                            /*{{ div(rownum-1, 100000)+1 }}*/
                        s_i_id       integer not null,
                            /*{{ mod(rownum-1, 100000)+1 }}*/
                        s_quantity   integer,
                            /*{{ rand.range_inclusive(10, 100) }}*/
                        s_ytd        integer,
                            /*{{ 0 }}*/
                        s_order_cnt  integer,
                            /*{{ 0 }}*/
                        s_remote_cnt integer,
                            /*{{ 0 }}*/
                        s_data       varchar(50),
                            /*{{
                                overlay(
                                    (@data := rand.regex('[0-9a-zA-Z]{26,50}'))
                                    placing
                                    (case when rand.bool(0.1) then 'ORIGINAL' else '' end)
                                    from
                                    (rand.range(1, octet_length(@data) - 7))
                                    using octets
                                )
                            }}*/
                        s_dist_01    char(24),
                            /*{{ rand.regex('[0-9a-zA-Z]{24}') }}*/
                        s_dist_02    char(24),
                            /*{{ rand.regex('[0-9a-zA-Z]{24}') }}*/
                        s_dist_03    char(24),
                            /*{{ rand.regex('[0-9a-zA-Z]{24}') }}*/
                        s_dist_04    char(24),
                            /*{{ rand.regex('[0-9a-zA-Z]{24}') }}*/
                        s_dist_05    char(24),
                            /*{{ rand.regex('[0-9a-zA-Z]{24}') }}*/
                        s_dist_06    char(24),
                            /*{{ rand.regex('[0-9a-zA-Z]{24}') }}*/
                        s_dist_07    char(24),
                            /*{{ rand.regex('[0-9a-zA-Z]{24}') }}*/
                        s_dist_08    char(24),
                            /*{{ rand.regex('[0-9a-zA-Z]{24}') }}*/
                        s_dist_09    char(24),
                            /*{{ rand.regex('[0-9a-zA-Z]{24}') }}*/
                        s_dist_10    char(24),
                            /*{{ rand.regex('[0-9a-zA-Z]{24}') }}*/
                        %(fk)sforeign key (s_w_id) references %(prefix)swarehouse (w_id),
                        %(fk)sforeign key (s_i_id) references %(prefix)sitem (i_id),
                        primary key (s_w_id, s_i_id)
                    );
                ||| % format,
            },

            /* 4_district

                10 rows in the DISTRICT table with:
                - D_ID           unique within [10]
                - D_W_ID       = W_ID
                - D_NAME         random a-string [6 .. 10]
                - D_STREET_n     random a-string [10 .. 20]
                - D_CITY         random a-string [10 .. 20]
                - D_STATE        random a-string of 2 letters
                - D_ZIP          generated according to Clause 4.3.2.7
                - D_TAX          random within [0.0000 .. 0.2000]
                - D_YTD        = 30,000.00
                - D_NEXT_O_ID  = 3,001

            10*W rows, 1.2 KiB/warehouse => 218,453 warehouses/file */
            m {
                out_dir+: '/4_district',
                seed: dbdbgen.xorSeed(m.seed, '012e6c79b1aca3877bfb75269efe5496a145ff2f2b38ce32fee151cce3cace63'),
                total_count: 10 * m.warehouses,
                rows_per_file: 200e3,
                template_string: |||
                    create table %(prefix)sdistrict (
                        d_w_id       integer not null,
                            /*{{ div(rownum-1, 10)+1 }}*/
                        d_id         integer not null,
                            /*{{ mod(rownum-1, 10)+1 }}*/
                        d_ytd        decimal(12,2),
                            /*{{ 30000.0 }}*/
                        d_tax        decimal(4,4),
                            /*{{ rand.range_inclusive(0, 2000)/10000 }}*/
                        d_next_o_id  integer,
                            /*{{ 3001 }}*/
                        d_name       varchar(10),
                            /*{{ rand.regex('[0-9a-zA-Z]{6,10}') }}*/
                        d_street_1   varchar(20),
                            /*{{ rand.regex('[0-9a-zA-Z]{10,20}') }}*/
                        d_street_2   varchar(20),
                            /*{{ rand.regex('[0-9a-zA-Z]{10,20}') }}*/
                        d_city       varchar(20),
                            /*{{ rand.regex('[0-9a-zA-Z]{10,20}') }}*/
                        d_state      char(2),
                            /*{{ rand.regex('[A-Z]{2}') }}*/
                        d_zip        char(9),
                            /*{{ rand.regex('[0-9]{4}11111') }}*/
                        %(fk)sforeign key (d_w_id) references %(prefix)swarehouse (w_id),
                        primary key (d_w_id, d_id)
                    );
                ||| % format,
            },

            /* 5_customer

                3,000 rows (per DISTRICT) in the CUSTOMER table with:

                - C_ID             unique within [3000]
                - C_D_ID         = D_ID
                - C_W_ID         = W_ID
                - C_LAST           generated according to Clause 4.3.2.3, iterating through the
                                   range of [0 .. 999] for the first 1,000 customers, and generating
                                   a non-uniform random number using the function NURand(255,0,999)
                                   for each of the remaining 2,000 customers.
                - C_MIDDLE       = "OE"
                - C_FIRST          random a-string [8 .. 16]
                - C_STREET_n       random a-string [10 .. 20]
                - C_CITY           random a-string [10 .. 20]
                - C_STATE          random a-string of 2 letters
                - C_ZIP            generated according to Clause 4.3.2.7
                - C_PHONE          random n-string of 16 numbers
                - C_SINCE          date/time given by the operating system when the CUSTOMER table
                                   was populated.
                - C_CREDIT       = "GC". For 10% of the rows, selected at random, C_CREDIT = "BC"
                - C_CREDIT_LIM   = 50,000.00
                - C_DISCOUNT       random within [0.0000 .. 0.5000]
                - C_BALANCE      = -10.00
                - C_YTD_PAYMENT  = 10.00
                - C_PAYMENT_CNT  = 1
                - C_DELIVERY_CNT = 0
                - C_DATA           random a-string [300 .. 500]

            30,000*W rows, 18 MiB/warehouse => 14 warehouses/file */
            m {
                out_dir+: '/5_customer',
                seed: dbdbgen.xorSeed(m.seed, '108ec395f96ca85edef8978e85f16b5c1840afa055a60b8202d6b49907983cdb'),
                total_count: 30e3 * m.warehouses,
                rows_per_file: 420e3,
                template_string: |||
                    /*{{
                        @last_names := array['BAR', 'OUGHT', 'ABLE', 'PRI', 'PRES', 'ESE', 'ANTI', 'CALLY', 'ATION', 'EING'];
                        @nurand_c := %(nurand_c)d
                    }}*/
                    create table %(prefix)scustomer (
                    c_w_id         integer not null,
                        /*{{ div(rownum-1, 30000)+1 }}*/
                    c_d_id         integer not null,
                        /*{{ mod(div(rownum-1, 3000), 10)+1 }}*/
                    c_id           integer not null,
                        /*{{ (@c_id := mod(rownum-1, 3000)) + 1 }}*/
                    c_discount     decimal(4,4),
                        /*{{ rand.range_inclusive(0, 5000)/10000 }}*/
                    c_credit       char(2),
                        /*{{ case when rand.bool(0.1) then 'BC' else 'GC' end }}*/
                    c_last         varchar(16),
                        /*{{
                            @index := case when @c_id >= 1000 then
                                mod((rand.range(0, 256) | rand.range(0, 1000)) + @nurand_c, 1000)
                            else
                                @c_id
                            end;
                            @last_names[div(@index, 100) + 1]
                                || @last_names[mod(div(@index, 10), 10) + 1]
                                || @last_names[mod(@index, 10) + 1]
                        }}*/
                    c_first        varchar(16),
                        /*{{ rand.regex('[0-9a-zA-Z]{8,16}') }}*/
                    c_credit_lim   decimal(12,2),
                        /*{{ 50000.0 }}*/
                    c_balance      decimal(12,2),
                        /*{{ -10.0 }}*/
                    c_ytd_payment  decimal(12,2),
                        /*{{ 10.0 }}*/
                    c_payment_cnt  integer,
                        /*{{ 1 }}*/
                    c_delivery_cnt integer,
                        /*{{ 0 }}*/
                    c_street_1     varchar(20),
                        /*{{ rand.regex('[0-9a-zA-Z]{10,20}') }}*/
                    c_street_2     varchar(20),
                        /*{{ rand.regex('[0-9a-zA-Z]{10,20}') }}*/
                    c_city         varchar(20),
                        /*{{ rand.regex('[0-9a-zA-Z]{10,20}') }}*/
                    c_state        char(2),
                        /*{{ rand.regex('[A-Z]{2}') }}*/
                    c_zip          char(9),
                        /*{{ rand.regex('[0-9]{4}11111') }}*/
                    c_phone        char(16),
                        /*{{ rand.regex('[0-9]{16}') }}*/
                    c_since        timestamp,
                        /*{{ current_timestamp }}*/
                    c_middle       char(2),
                        /*{{ 'OE' }}*/
                    c_data         varchar(500),
                        /*{{ rand.regex('[0-9a-zA-Z]{300,500}') }}*/
                    %(fk)sforeign key (c_w_id, c_d_id) references %(prefix)sdistrict (d_w_id, d_id),
                    primary key (c_w_id, c_d_id, c_id)
                    );
                ||| % format,
            },

            /* 6_history

                1 row (per CUSTOMER) in the HISTORY table with:

                - H_C_ID   = C_ID
                - H_C_D_ID = H_D_ID = D_ID
                - H_C_W_ID = H_W_ID = W_ID
                - H_DATE     current date and time
                - H_AMOUNT = 10.00
                - H_DATA     random a-string [12 .. 24]

            30,000*W rows, 2.4 MiB/warehouse => 100 warehouses/file */
            m {
                out_dir+: '/6_history',
                seed: dbdbgen.xorSeed(m.seed, 'c4c573c43582c458f3b6468b088ac71ad11ba0a9c0e89567f2850d113dfda6ef'),
                total_count: 30e3 * m.warehouses,
                rows_per_file: 3e6,
                template_string: |||
                    create table %(prefix)shistory (
                        h_c_id   integer,
                            /*{{ mod(rownum-1, 3000)+1 }}*/
                        h_c_d_id integer,
                            /*{{ @d_id := mod(div(rownum-1, 3000), 10)+1 }}*/
                        h_c_w_id integer,
                            /*{{ @w_id := div(rownum-1, 30000)+1 }}*/
                        h_d_id   integer,
                            /*{{ @d_id }}*/
                        h_w_id   integer,
                            /*{{ @w_id }}*/
                        h_date   timestamp,
                            /*{{ current_timestamp }}*/
                        h_amount decimal(6,2),
                            /*{{ 10.0 }}*/
                        h_data   varchar(24)
                            /*{{ rand.regex('[0-9a-zA-Z]{12,24}') }}*/
                        %(fk)s,foreign key (h_c_w_id, h_c_d_id, h_c_id) references %(prefix)scustomer (c_w_id, c_d_id, c_id)
                        %(fk)s,foreign key (h_w_id, h_d_id) references %(prefix)sdistrict (d_w_id, d_id)
                    );
                ||| % format,
            },

            /* 7_order

                3,000 rows (per DISTRICT) in the ORDER table with:

                - O_ID           unique within [3,000]
                - O_C_ID         selected sequentially from a random permutation of [1 .. 3,000]
                - O_D_ID       = D_ID
                - O_W_ID       = W_ID
                - O_ENTRY_D      current date/time given by the operating system
                - O_CARRIER_ID   random within [1 .. 10] if O_ID < 2,101, null otherwise
                - O_OL_CNT       random within [5 .. 15]
                - O_ALL_LOCAL  = 1

                A number of rows in the ORDER-LINE table equal to O_OL_CNT, generated according to
                the rules for input data generation of the New-Order transaction (see Clause 2.4.1)
                with:

                - OL_O_ID        = O_ID
                - OL_D_ID        = D_ID
                - OL_W_ID        = W_ID
                - OL_NUMBER        unique within [O_OL_CNT]
                - OL_I_ID          random within [1 .. 100,000]
                - OL_SUPPLY_W_ID = W_ID
                - OL_DELIVERY_D  = O_ENTRY_D if OL_O_ID < 2,101, null otherwise
                - OL_QUANTITY    = 5
                - OL_AMOUNT      = 0.00 if OL_O_ID < 2,101, random within [0.01 .. 9,999.99] otherwise
                - OL_DIST_INFO     random a-string of 24 letters

            30,000*W rows (order), 1.8 MiB/warehouse
            ~300,000*W rows (order_line), 25 MiB/warehouse => 10 warehouses/file */
            m {
                out_dir+: '/7_order',
                seed: dbdbgen.xorSeed(m.seed, '536cc39a9d07861acc0aec7458a7a52c3bd1815ddc2ba9eabd6982d767b85430'),
                total_count: 30e3 * m.warehouses,
                rows_per_file: 300e3,
                template_string: |||
                    /*{{ @c_ids := generate_series(1, 3000) }}*/
                    create table %(prefix)soorder (
                        o_w_id       integer not null,
                            /*{{ @w_id := div(rownum-1, 30000)+1 }}*/
                        o_d_id       integer not null,
                            /*{{ @d_id := mod(div(rownum-1, 3000), 10)+1 }}*/
                        o_id         integer not null,
                            /*{{ @o_id := mod(rownum-1, 3000)+1 }}*/
                        o_c_id       integer,
                            /*{{
                                (case @o_id
                                    when 1 then @c_ids := rand.shuffle(@c_ids)
                                    else @c_ids
                                end)[@o_id]
                            }}*/
                        o_carrier_id integer,
                            /*{{ case when @o_id <= 2100 then rand.range_inclusive(1, 10) end }}*/
                        o_ol_cnt     integer,
                            /*{{ @ol_cnt := rand.range_inclusive(5, 15) }}*/
                        o_all_local  integer,
                            /*{{ 1 }}*/
                        o_entry_d    timestamp,
                            /*{{ @o_entry_d := current_timestamp }}*/
                        %(fk)sforeign key (o_w_id, o_d_id, o_c_id) references %(prefix)scustomer (c_w_id, c_d_id, c_id),
                        primary key (o_w_id, o_d_id, o_id)
                    );

                    /*{{ for each row of %(prefix)soorder generate @ol_cnt rows of %(prefix)sorder_line }}*/
                    create table %(prefix)sorder_line (
                        ol_w_id         integer not null,
                            /*{{ @w_id }}*/
                        ol_d_id         integer not null,
                            /*{{ @d_id }}*/
                        ol_o_id         integer not null,
                            /*{{ @o_id }}*/
                        ol_number       integer not null,
                            /*{{ subrownum }}*/
                        ol_i_id         integer not null,
                            /*{{ rand.range_inclusive(1, 100000) }}*/
                        ol_delivery_d   timestamp,
                            /*{{ case when @o_id <= 2100 then @o_entry_d end }}*/
                        ol_amount       decimal(6,2),
                            /*{{
                                case when @o_id <= 2100 then
                                    0.0
                                else
                                    rand.range(1, 1000000)/100
                                end
                            }}*/
                        ol_supply_w_id  integer,
                            /*{{ @w_id }}*/
                        ol_quantity     integer,
                            /*{{ 5 }}*/
                        ol_dist_info    char(24),
                            /*{{ rand.regex('[0-9a-zA-Z]{24}') }}*/
                        %(fk)sforeign key (ol_w_id, ol_d_id, ol_o_id) references %(prefix)soorder (o_w_id, o_d_id, o_id),
                        %(fk)sforeign key (ol_supply_w_id, ol_i_id) references %(prefix)sstock (s_w_id, s_i_id),
                        primary key (ol_w_id, ol_d_id, ol_o_id, ol_number)
                    );
                ||| % format,
            },

            /* 8_new_order

                900 rows (per DISTRICT) in the NEW-ORDER table corresponding to the last 900 rows in
                the ORDER table for that district (i.e., with NO_O_ID between 2,101 and 3,000), with:

                - NO_O_ID = O_ID
                - NO_D_ID = D_ID
                - NO_W_ID = W_ID

            9000*W rows, 127 KiB/warehouse => 2048 warehouses/file */
            m {
                out_dir+: '/8_new_order',
                seed: dbdbgen.xorSeed(m.seed, '76272cbaac3e823ea217e9f51545034e43cf2a4864cbedf74fa88defc1afe5a5'),
                total_count: 9e3 * m.warehouses,
                rows_per_file: 18e6,
                template_string: |||
                    create table %(prefix)snew_order (
                        no_w_id  integer not null,
                            /*{{ div(rownum-1, 9000)+1 }}*/
                        no_d_id  integer not null,
                            /*{{ mod(div(rownum-1, 900), 10)+1 }}*/
                        no_o_id  integer not null,
                            /*{{ mod(rownum-1, 900)+2101 }}*/
                        %(fk)sforeign key (no_w_id, no_d_id, no_o_id) references %(prefix)soorder (o_w_id, o_d_id, o_id),
                        primary key (no_w_id, no_d_id, no_o_id)
                    );
                ||| % format,
            },
        ]
}
