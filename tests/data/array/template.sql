CREATE TABLE result (
    empty   {{ array[] }}
    one     {{ @a := array[1] }}
    two     {{ array[1, 2] }}
    nested  {{ array[array[], array[3], array[4, 5]] }}

    compare_1st_elem    {{ array[3, 6] < array[4, 1] }}
    compare_2nd_elem    {{ array[3, 6] < array[3, 1] }}
    compare_longer      {{ array[3, 6] < array[3] }}
    compare_equal       {{ array[3, 6] < array[3, 6] }}
    compare_shorter     {{ array[3, 6] < array[3, 6, 9] }}

    elem_1      {{ array[10, 20, 30][1] }}
    elem_3      {{ array[10, 20, 30][3] }}
    elem_0      {{ array[10, 20, 30][0] }}
    elem_999    {{ array[10, 20, 30][999] }}
    nested_elem {{ array[array[13]][1][1] }}

    array_var   {{ -@a[1] }}

    gs_pos_step         {{ generate_series(11, 21, 5) }}
    gs_pos_step_empty   {{ generate_series(21, 11, 5) }}
    gs_neg_step_empty   {{ generate_series(11, 21, -5) }}
    gs_neg_step         {{ generate_series(21, 11, -5) }}
    gs_pos_float        {{ generate_series(1.1, 2.25, 0.5) }}
    gs_neg_float        {{ generate_series(2.25, 1.1, -0.5) }}
    gs_implicit         {{ generate_series(1, 4) }}
    gs_implicit_single  {{ generate_series(3, 3) }}
    gs_step_too_large   {{ generate_series(4, 5, 7) }}
    gs_timestamp        {{ generate_series(TIMESTAMP '2019-01-01 13:00:00', TIMESTAMP '2019-01-01 14:00:00', INTERVAL 20 MINUTE) }}
    gs_elem_3           {{ generate_series(11, 21, 5)[3] }}
    gs_elem_999         {{ generate_series(11, 21, 5)[999] }}

    shuffle_empty       {{ rand.shuffle(array[]) }}
    shuffle_one         {{ rand.shuffle(array['one']) }}
    shuffle_elem_1      {{ rand.shuffle(@a)[1] }}
    shuffle_elem_999    {{ rand.shuffle(@a)[999] }}

    gs_compare_concrete {{ generate_series(11, 21, 5) = array[11, 16, 21] }}
);
