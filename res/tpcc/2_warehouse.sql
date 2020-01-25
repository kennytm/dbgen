create table bmsql_warehouse (
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

/* Configure WAREHOUSE table according to TPC-C v5.11 §4.3.3.1:

1 row in the WAREHOUSE table for each configured warehouse with:
 - W_ID       unique within [number_of_configured_warehouses]
 - W_NAME     random a-string [6 .. 10]
 - W_STREET_n random a-string [10 .. 20]
 - W_CITY     random a-string [10 .. 20]
 - W_STATE    random a-string of 2 letters
 - W_ZIP      generated according to Clause 4.3.2.7
 - W_TAX      random within [0.0000 .. 0.2000]
 - W_YTD    = 300,000.00
*/

