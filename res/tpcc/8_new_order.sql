create table bmsql_new_order (
  no_w_id  integer not null,
    /*{{ div(rownum-1, 9000)+1 }}*/
  no_d_id  integer not null,
    /*{{ mod(div(rownum-1, 900), 10)+1 }}*/
  no_o_id  integer not null,
    /*{{ mod(rownum-1, 900)+2101 }}*/
  primary key (no_w_id, no_d_id, no_o_id),
  foreign key (no_w_id, no_d_id, no_o_id) references bmsql_oorder (o_w_id, o_d_id, o_id)
);

/* Configure NEW-ORDER table according to TPC-C v5.11 §4.3.3.1:

900 rows in the NEW-ORDER table corresponding to the last 900 rows in the ORDER
table for that district (i.e., with NO_O_ID between 2,101 and 3,000), with:

 - NO_O_ID = O_ID
 - NO_D_ID = D_ID
 - NO_W_ID = W_ID
*/
