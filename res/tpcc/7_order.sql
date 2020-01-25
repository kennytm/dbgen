/*{{ @c_ids := generate_series(1, 3000) }}*/
create table bmsql_oorder (
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
  primary key (o_w_id, o_d_id, o_id)
  -- foreign key (o_w_id, o_d_id, o_c_id) references bmsql_customer (c_w_id, c_d_id, c_id)
);

/* Configure ORDER table according to TPC-C v5.11 §4.3.3.1:

3,000 rows in the ORDER table with:

 - O_ID           unique within [3,000]
 - O_C_ID         selected sequentially from a random permutation of [1 .. 3,000]
 - O_D_ID       = D_ID
 - O_W_ID       = W_ID
 - O_ENTRY_D      current date/time given by the operating system
 - O_CARRIER_ID   random within [1 .. 10] if O_ID < 2,101, null otherwise
 - O_OL_CNT       random within [5 .. 15]
 - O_ALL_LOCAL  = 1
*/

/*{{ for each row of bmsql_oorder generate @ol_cnt rows of bmsql_order_line }}*/
create table bmsql_order_line (
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
  primary key (ol_w_id, ol_d_id, ol_o_id, ol_number)
  -- foreign key (ol_w_id, ol_d_id, ol_o_id) references bmsql_oorder (o_w_id, o_d_id, o_id),
  -- foreign key (ol_supply_w_id, ol_i_id) references bmsql_stock (s_w_id, s_i_id)
);

/* Configure ORDER-LINE table according to TPC-C v5.11 §4.3.3.1:

A number of rows in the ORDER-LINE table equal to O_OL_CNT, generated according to the rules for
input data generation of the New-Order transaction (see Clause 2.4.1) with:

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
*/
