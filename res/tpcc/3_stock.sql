create table bmsql_stock (
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
  primary key (s_w_id, s_i_id),
  foreign key (s_w_id) references bmsql_warehouse (w_id),
  foreign key (s_i_id) references bmsql_item (i_id)
);

/* Configure STOCK table according to TPC-C v5.11 §4.3.3.1:

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
*/
