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
);
