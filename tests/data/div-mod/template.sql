create table result (
    div_int_pos {{ div(9, 4*(rownum - 2)) }}
    mod_int_pos {{ mod(9, 4*(rownum - 2)) }}
    div_int_neg {{ div(-9, 4*(rownum - 2)) }}
    mod_int_neg {{ mod(-9, 4*(rownum - 2)) }}
    div_float_pos   {{ div(9.7, 4.1*(rownum - 2)) }}
    mod_float_pos   {{ mod(9.7, 4.1*(rownum - 2)) }}
    div_float_neg   {{ div(-9.7, 4.1*(rownum - 2)) }}
    mod_float_neg   {{ mod(-9.7, 4.1*(rownum - 2)) }}
    float_div_int   {{ 9 / (4*(rownum - 2)) }}
    float_div_float {{ 9.7 / (3.6*(rownum - 2)) }}
);
