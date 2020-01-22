{{ @a := 1 + 2 + 3 }}
{{ @b := @a * 2 }}
CREATE TABLE result (
    {{ @c := rownum + @a + @b }}
    {{ @c + @d }}
);
