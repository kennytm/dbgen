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
);
