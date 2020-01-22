CREATE TABLE result (
    {{ array[] }}
    {{ @a := array[1] }}
    {{ array[1, 2] }}
    {{ array[array[], array[3], array[4, 5]] }}

    {{ array[3, 6] < array[4, 1] }}
    {{ array[3, 6] < array[3, 1] }}
    {{ array[3, 6] < array[3] }}
    {{ array[3, 6] < array[3, 6] }}
    {{ array[3, 6] < array[3, 6, 9] }}

    {{ array[10, 20, 30][1] }}
    {{ array[10, 20, 30][3] }}
    {{ array[10, 20, 30][0] }}
    {{ array[10, 20, 30][999] }}
    {{ array[array[13]][1][1] }}

    {{ -@a[1] }}

    {{ generate_series(11, 21, 5) }}
    {{ generate_series(21, 11, 5) }}
    {{ generate_series(11, 21, -5) }}
    {{ generate_series(21, 11, -5) }}
    {{ generate_series(1.1, 2.25, 0.5) }}
    {{ generate_series(2.25, 1.1, -0.5) }}
    {{ generate_series(1, 4) }}
    {{ generate_series(3, 3) }}
    {{ generate_series(4, 5, 7) }}
    {{ generate_series(TIMESTAMP '2019-01-01 13:00:00', TIMESTAMP '2019-01-01 14:00:00', INTERVAL 20 MINUTE) }}
);
