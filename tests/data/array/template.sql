CREATE TABLE result (
    {{ array[] }}
    {{ array[1] }}
    {{ array[1, 2] }}
    {{ array[array[], array[3], array[4, 5]] }}

    {{ array[3, 6] < array[4, 1] }}
    {{ array[3, 6] < array[3, 1] }}
    {{ array[3, 6] < array[3] }}
    {{ array[3, 6] < array[3, 6] }}
    {{ array[3, 6] < array[3, 6, 9] }}
);
