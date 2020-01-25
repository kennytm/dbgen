/*{{
@last_names := array['BAR', 'OUGHT', 'ABLE', 'PRI', 'PRES', 'ESE', 'ANTI', 'CALLY', 'ATION', 'EING'];
@nurand_c := coalesce(@nurand_c, rand.range(0, 256))
}}*/
create table bmsql_customer (
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
  primary key (c_w_id, c_d_id, c_id),
  foreign key (c_w_id, c_d_id) references bmsql_district (d_w_id, d_id)
);

/* Configure CUSTOMER table according to TPC-C v5.11 §4.3.3.1:

3,000 rows in the CUSTOMER table with:

 - C_ID             unique within [3000]
 - C_D_ID         = D_ID
 - C_W_ID         = W_ID
 - C_LAST           generated according to Clause 4.3.2.3, iterating through the
                    range of [0 .. 999] for the first 1,000 customers, and
                    generating a non-uniform random number using the function
                    NURand(255,0,999) for each of the remaining 2,000 customers.
 - C_MIDDLE       = "OE"
 - C_FIRST          random a-string [8 .. 16]
 - C_STREET_n       random a-string [10 .. 20]
 - C_CITY           random a-string [10 .. 20]
 - C_STATE          random a-string of 2 letters
 - C_ZIP            generated according to Clause 4.3.2.7
 - C_PHONE          random n-string of 16 numbers
 - C_SINCE          date/time given by the operating system when the CUSTOMER
                    table was populated.
 - C_CREDIT       = "GC". For 10% of the rows, selected at random,
                    C_CREDIT = "BC"
 - C_CREDIT_LIM   = 50,000.00
 - C_DISCOUNT       random within [0.0000 .. 0.5000]
 - C_BALANCE      = -10.00
 - C_YTD_PAYMENT  = 10.00
 - C_PAYMENT_CNT  = 1
 - C_DELIVERY_CNT = 0
 - C_DATA           random a-string [300 .. 500]
*/

