{{ @short_array := generate_series(1001, 1500) }}
{{ @shuffled_short_array := rand.shuffle(@short_array) }}
{{ @long_array := generate_series(1, 10000000000000000000) }}
{{ @shuffled_long_array := rand.shuffle(@long_array) }}

CREATE TABLE result (
    {{ rownum }}
    {{ @short_array[rownum] }}
    {{ @shuffled_short_array[rownum] }}
    {{ @long_array[20000000000000000 * rownum] }}
    {{ @shuffled_long_array[20000000000000000 * rownum] }}
)