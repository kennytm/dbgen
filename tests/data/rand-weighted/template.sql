CREATE TABLE result (
    {{ rownum }}
    {{ rand.weighted(array[2, 3, 5]) }}
);
