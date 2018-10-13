CREATE TABLE result (
    {{ rownum }}
    {{ rand.range(0, 10) }}
    {{ rand.zipf(10, 0.75) }}
);
