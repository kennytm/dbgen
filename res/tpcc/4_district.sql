create table bmsql_district (
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
  primary key (d_w_id, d_id),
  foreign key (d_w_id) references bmsql_warehouse (w_id)
);

/* Configure DISTRICT table according to TPC-C v5.11 §4.3.3.1:

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
*/
