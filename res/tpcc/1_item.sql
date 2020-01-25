create table bmsql_item (
  i_id     integer primary key,
    /*{{ rownum }}*/
  i_name   varchar(24),
    /*{{ rand.regex('[0-9a-zA-Z]{14,24}') }}*/
  i_price  decimal(5,2),
    /*{{ rand.range_inclusive(100, 10000)/100 }}*/
  i_data   varchar(50),
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
  i_im_id  integer
    /*{{ rand.range_inclusive(1, 10000) }}*/
);

/* Configure ITEM table according to TPC-C v5.11 §4.3.3.1:

100,000 rows in the ITEM table with:
 - I_ID     unique within [100,000]
 - I_IM_ID  random within [1 .. 10,000]
 - I_NAME   random a-string [14 .. 24]
 - I_PRICE  random within [1.00 .. 100.00]
 - I_DATA   random a-string [26 .. 50]. For 10% of the rows, selected at random,
              the string "ORIGINAL" must be held by 8 consecutive characters
              starting at random position within I_DATA
*/

