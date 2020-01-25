create table bmsql_history (
  hist_id  serial primary key,
    /*{{ rownum }}*/
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
  h_data   varchar(24),
    /*{{ rand.regex('[0-9a-zA-Z]{12,24}') }}*/
  foreign key (h_c_w_id, h_c_d_id, h_c_id) references bmsql_customer (c_w_id, c_d_id, c_id),
  foreign key (h_w_id, h_d_id) references bmsql_district (d_w_id, d_id)
);

/* Configure HISTORY table according to TPC-C v5.11 §4.3.3.1:

1 row in the HISTORY table with:

 - H_C_ID   = C_ID
 - H_C_D_ID = H_D_ID = D_ID
 - H_C_W_ID = H_W_ID = W_ID
 - H_DATE     current date and time
 - H_AMOUNT = 10.00
 - H_DATA     random a-string [12 .. 24]

*/
