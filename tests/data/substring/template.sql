create table result (
    {{ substring('🥰😘😍' from 1) }}
    {{ substring('🥰😘😍' from 2) }}
    {{ substring('🥰😘😍' from 2 for 1) }}
    {{ substring('🥰😘😍' from -99) }}
    {{ substring('🥰😘😍' from 99) }}
    {{ substring('🥰😘😍' from 2 for 99) }}
    {{ substring('🥰😘😍' from -2 for 99) }}
    {{ substring('🥰😘😍' from -1 for 3) }}
    {{ substring('🥰😘😍' from 2 for -1) }}
    {{ substring('🥰' from 2 using octets) }}
    {{ substring('🥰' from 2 for 2 using octets) }}
    {{ substring('🥰' from -1 for 3 using octets) }}
    {{ substring('🥰' from 99 using octets) }}
    {{ substring('🥰' from 99 for 99 using octets) }}
    {{ substring('🥰😘😍' for 2) }}
    {{ substring('🥰😘😍' for 2 using octets) }}

    {{ overlay('ABCDEF' placing '🥰' from 2) }}
    {{ overlay('ABCDEF' placing '🥰' from 2 using octets) }}
    {{ overlay('🥰😘😍' placing 'A' from 1) }}
    {{ overlay('🥰' placing 'A' from 1 using octets) }}
    {{ overlay('XYZ' placing 'abc' from 3) }}
    {{ overlay('XYZ' placing 'abc' from 3 using octets) }}
    {{ overlay('ABCDEF' placing '_' from 2 for 4) }}
    {{ overlay('ABCDEF' placing '_' from 2 for 0) }}
);
