create table result (
    ss_a {{ substring('🥰😘😍' from 1) }}
    ss_b {{ substring('🥰😘😍' from 2) }}
    ss_c {{ substring('🥰😘😍' from 2 for 1) }}
    ss_d {{ substring('🥰😘😍' from -99) }}
    ss_e {{ substring('🥰😘😍' from 99) }}
    ss_f {{ substring('🥰😘😍' from 2 for 99) }}
    ss_g {{ substring('🥰😘😍' from -2 for 99) }}
    ss_h {{ substring('🥰😘😍' from -1 for 3) }}
    ss_i {{ substring('🥰😘😍' from 2 for -1) }}
    ss_j {{ substring('🥰' from 2 using octets) }}
    ss_k {{ substring('🥰' from 2 for 2 using octets) }}
    ss_l {{ substring('🥰' from -1 for 3 using octets) }}
    ss_m {{ substring('🥰' from 99 using octets) }}
    ss_n {{ substring('🥰' from 99 for 99 using octets) }}
    ss_o {{ substring('🥰😘😍' for 2) }}
    ss_p {{ substring('🥰😘😍' for 2 using octets) }}

    ov_a {{ overlay('ABCDEF' placing '🥰' from 2) }}
    ov_b {{ overlay('ABCDEF' placing '🥰' from 2 using octets) }}
    ov_c {{ overlay('🥰😘😍' placing 'A' from 1) }}
    ov_d {{ overlay('🥰' placing 'A' from 1 using octets) }}
    ov_e {{ overlay('XYZ' placing 'abc' from 3) }}
    ov_f {{ overlay('XYZ' placing 'abc' from 3 using octets) }}
    ov_g {{ overlay('ABCDEF' placing '_' from 2 for 4) }}
    ov_h {{ overlay('ABCDEF' placing '_' from 2 for 0) }}
);
