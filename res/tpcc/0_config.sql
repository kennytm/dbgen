/*{{
    @names := array['warehouses', 'nURandCLast', 'nURandCC_ID', 'nURandCI_ID'];
    @values := array[@warehouses, @nurand_c, rand.range(0, 1024), rand.range(0, 8192)]
}}*/
CREATE TABLE bmsql_config (
    cfg_name    varchar(30) primary key,
        /*{{ @names[rownum] }}*/
    cfg_value   varchar(50)
        /*{{ @values[rownum] || '' }}*/
);
