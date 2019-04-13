CREATE TABLE result (
    {{ 1 }}
    {{ 2 + 5 }}
    {{ 3 - 8 }}
    {{ 7 * 6 }}
    {{ 12 / 24 }}
    {{ - - - - 3 }}
    {{ - 3 }}

    {{ TRUE AND TRUE }} {{ TRUE AND FALSE }} {{ TRUE AND NULL }}
    {{ FALSE AND TRUE }} {{ FALSE AND FALSE }} {{ FALSE AND NULL }}
    {{ NULL AND TRUE }} {{ NULL AND FALSE }} {{ NULL AND NULL }}

    {{ TRUE OR TRUE }} {{ TRUE OR FALSE }} {{ TRUE OR NULL }}
    {{ FALSE OR TRUE }} {{ FALSE OR FALSE }} {{ FALSE OR NULL }}
    {{ NULL OR TRUE }} {{ NULL OR FALSE }} {{ NULL OR NULL }}

    {{ TRUE IS TRUE }} {{ TRUE IS FALSE }} {{ TRUE IS NULL }}
    {{ FALSE IS TRUE }} {{ FALSE IS FALSE }} {{ FALSE IS NULL }}
    {{ NULL IS TRUE }} {{ NULL IS FALSE }} {{ NULL IS NULL }}

    {{ TRUE IS NOT TRUE }} {{ TRUE IS NOT FALSE }} {{ TRUE IS NOT NULL }}
    {{ FALSE IS NOT TRUE }} {{ FALSE IS NOT FALSE }} {{ FALSE IS NOT NULL }}
    {{ NULL IS NOT TRUE }} {{ NULL IS NOT FALSE }} {{ NULL IS NOT NULL }}

    {{ NOT TRUE }} {{ NOT FALSE }} {{ NOT NULL }}

    {{  0xffffffffffffffff }}
    {{ -0x8000000000000001 }}

    {{ 1.5 }}
    {{ 1.5e300 }}
    {{ 1e300 }}
    {{ .5e300 }}
    {{ 5.e-250 }}
    {{ 6e+10 }}

    {{ 'hello world' }}
    {{ 'hello' || ', ' || 'world!!' || 111 }}
    {{ '👋' || '🌍' }}

    {{ greatest(1, 3, 2, 9, 6, 0, 5) }}
    {{ least(1, 3, 2, 9, 6, 0, 5) }}

    {{ case 6
        when 1 then 'one'
        when 3 then 'three'
        when 6 then 'six'
        when 10 then 'ten'
    end }}
    {{ case 5
        when 1 then 'one'
        when 3 then 'three'
        when 6 then 'six'
        when 10 then 'ten'
    end }}
    {{ case 4
        when 1 then 'one'
        when 3 then 'three'
        when 6 then 'six'
        when 10 then 'ten'
        else 'otherwise'
    end }}
    {{ case 3
        when 1 then 'one'
        when 3 then 'three'
        when 6 then 'six'
        when 10 then 'ten'
        else 'otherwise'
    end }}

    {{ ((((((((((((((((((((((((7)))))))))))))))))))))))) }}
    {{ 1 + 2 - 3 - 4 + 5 + 6 + 7 - 8 - 9 - 10 - 11 }}
    {{ 1 / 0 }}

    {{ 60 > 3 }}
    {{ 60 < 3 }}
    {{ 60 >= 3 }}
    {{ 60 <= 3 }}
    {{ 60 = 3 }}
    {{ 60 <> 3 }}

    {{ @a := 18 }}
    {{ @a }}

    {{ @b := @c := @d := 'e' }}
    {{ @a || @b || @c || @d }}

    {{ timestamp '2010-01-01 00:00:00' }}
    {{ timestamp '2010-01-01 00:00:00.000001' }}
    {{ timestamp '2010-01-01 00:00:00' + interval 1 microsecond }}
    {{ timestamp '2010-01-01 00:00:00' + interval 1 microsecond = timestamp '2010-01-01 00:00:00.000001' }}
    {{ timestamp '2010-01-01 00:00:00' - interval 4 week }}
    {{ timestamp '2010-01-01 00:00:00' - interval 3.5 day * 12 }}
    {{ timestamp '2010-01-01 00:00:00' + interval 15 hour + interval 71 minute - interval 13 second }}

    {{ '\' }}

    {{ round(123.45) }}
    {{ round(123.45, 1) }}
    {{ round(-123.975, 2) }}
    {{ round(123.456, 9) }}
    {{ round(123.456, -1) }}
    {{ round(123.456, -9) }}

    {{ interval 0 microsecond }}
    {{ interval 1234567890 microsecond }}
    {{ interval -1234567890 microsecond }}
    {{ interval 1234567890 second }}
);
