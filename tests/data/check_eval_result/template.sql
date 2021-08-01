CREATE TABLE result (
    number  {{ 1 }}
    add     {{ 2 + 5 }}
    sub     {{ 3 - 8 }}
    mul     {{ 7 * 6 }}
    div     {{ 12 / 24 }}
    neg_4   {{ - - - - 3 }}
    neg_1   {{ - 3 }}

    and_11  {{ TRUE AND TRUE }}
    and_10  {{ TRUE AND FALSE }}
    and_1n  {{ TRUE AND NULL }}
    and_01  {{ FALSE AND TRUE }}
    and_00  {{ FALSE AND FALSE }}
    and_0n  {{ FALSE AND NULL }}
    and_n1  {{ NULL AND TRUE }}
    and_n0  {{ NULL AND FALSE }}
    and_nn  {{ NULL AND NULL }}

    or_11   {{ TRUE OR TRUE }}
    or_10   {{ TRUE OR FALSE }}
    or_1n   {{ TRUE OR NULL }}
    or_01   {{ FALSE OR TRUE }}
    or_00   {{ FALSE OR FALSE }}
    or_0n   {{ FALSE OR NULL }}
    or_n1   {{ NULL OR TRUE }}
    or_n0   {{ NULL OR FALSE }}
    or_nn   {{ NULL OR NULL }}

    is_11   {{ TRUE IS TRUE }}
    is_10   {{ TRUE IS FALSE }}
    is_1n   {{ TRUE IS NULL }}
    is_01   {{ FALSE IS TRUE }}
    is_00   {{ FALSE IS FALSE }}
    is_0n   {{ FALSE IS NULL }}
    is_n1   {{ NULL IS TRUE }}
    is_n0   {{ NULL IS FALSE }}
    is_nn   {{ NULL IS NULL }}

    is_not_11   {{ TRUE IS NOT TRUE }}
    is_not_10   {{ TRUE IS NOT FALSE }}
    is_not_1n   {{ TRUE IS NOT NULL }}
    is_not_01   {{ FALSE IS NOT TRUE }}
    is_not_00   {{ FALSE IS NOT FALSE }}
    is_not_0n   {{ FALSE IS NOT NULL }}
    is_not_n1   {{ NULL IS NOT TRUE }}
    is_not_n0   {{ NULL IS NOT FALSE }}
    is_not_nn   {{ NULL IS NOT NULL }}

    not_true    {{ NOT TRUE }}
    not_false   {{ NOT FALSE }}
    not_null    {{ NOT NULL }}

    u64_max     {{  0xffffffffffffffff }}
    neg_i64_min {{ -0x8000000000000001 }}

    float_normal    {{ 1.5 }}
    float_e300      {{ 1.5e300 }}
    float_no_dot    {{ 1e300 }}
    float_no_zero   {{ .5e300 }}
    float_dot_e     {{ 5.e-250 }}
    float_e_plus    {{ 6e+10 }}

    string          {{ 'hello world' }}
    string_concat   {{ 'hello' || ', ' || 'world!!' || 111 }}
    string_emoji    {{ '👋' || '🌍' }}

    greatest    {{ greatest(1, 3, 2, 9, 6, 0, 5) }}
    least       {{ least(1, 3, 2, 9, 6, 0, 5) }}

    case_6 {{ case 6
        when 1 then 'one'
        when 3 then 'three'
        when 6 then 'six'
        when 10 then 'ten'
    end }}
    case_5 {{ case 5
        when 1 then 'one'
        when 3 then 'three'
        when 6 then 'six'
        when 10 then 'ten'
    end }}
    case_4 {{ case 4
        when 1 then 'one'
        when 3 then 'three'
        when 6 then 'six'
        when 10 then 'ten'
        else 'otherwise'
    end }}
    case_3 {{ case 3
        when 1 then 'one'
        when 3 then 'three'
        when 6 then 'six'
        when 10 then 'ten'
        else 'otherwise'
    end }}
    case_cond {{ case
        when null then 'null'
        when false then 'false'
        when -3 then 'minus three'
        when true then 'true'
    end }}

    parethensis     {{ ((((((((((((((((((((((((7)))))))))))))))))))))))) }}
    chain_add_sub   {{ 1 + 2 - 3 - 4 + 5 + 6 + 7 - 8 - 9 - 10 - 11 }}
    div_by_0        {{ 1 / 0 }}

    gt  {{ 60 > 3 }}
    lt  {{ 60 < 3 }}
    ge  {{ 60 >= 3 }}
    le  {{ 60 <= 3 }}
    eq  {{ 60 = 3 }}
    ne  {{ 60 <> 3 }}

    var_def     {{ @a := 18 }}
    var_use     {{ @a }}

    chain_def   {{ @b := @c := @d := 'e' }}
    chain_use   {{ @a || @b || @c || @d }}

    ts_normal   {{ timestamp '2010-01-01 00:00:00' }}
    ts_frac     {{ timestamp '2010-01-01 00:00:00.000001' }}
    ts_add      {{ timestamp '2010-01-01 00:00:00' + interval 1 microsecond }}
    ts_compare  {{ timestamp '2010-01-01 00:00:00' + interval 1 microsecond = timestamp '2010-01-01 00:00:00.000001' }}
    ts_sub      {{ timestamp '2010-01-01 00:00:00' - interval 4 week }}
    ts_mul_iv   {{ timestamp '2010-01-01 00:00:00' - interval 3.5 day * 12 }}
    ts_add_iv   {{ timestamp '2010-01-01 00:00:00' + interval 15 hour + interval 71 minute - interval 13 second }}

    backslash   {{ '\' }}

    round_0     {{ round(123.45) }}
    round_1     {{ round(123.45, 1) }}
    round_2     {{ round(-123.975, 2) }}
    round_9     {{ round(123.456, 9) }}
    round_neg_1 {{ round(123.456, -1) }}
    round_neg_9 {{ round(123.456, -9) }}

    interval_0      {{ interval 0 microsecond }}
    interval_pos    {{ interval 1234567890 microsecond }}
    interval_neg    {{ interval -1234567890 microsecond }}
    interval_big    {{ interval 1234567890 second }}

    chain_and   {{ true and true and false }}
    chain_or    {{ false or false or true }}
    chain_add   {{ 5 + 6 + 7 }}
    chain_mul   {{ 5 * 6 * 7 }}
    chain_sub   {{ 5 - 6 - 7 }}
    chain_div   {{ 7 / 4 / 2 }}

    char_length     {{ char_length('Unicodeの文字集合の符号空間は0–10FFFF₁₆で111万4112符号位置がある。') }}
    character_length{{ character_length('Unicodeの文字集合の符号空間は0–10FFFF₁₆で111万4112符号位置がある。') }}
    octet_length    {{ octet_length('Unicodeの文字集合の符号空間は0–10FFFF₁₆で111万4112符号位置がある。') }}

    coalesce_12 {{ coalesce(1, 2) }}
    coalesce_1n {{ coalesce(1, null) }}
    coalesce_n2 {{ coalesce(null, 2) }}
    coalesce_nn {{ coalesce(null, null) }}

    semicolon   {{ @e := 567; @f := @e - 7; @f + 40 }}

    bit_and {{ 80 & 91 & 68 }}
    bit_or  {{ 80 | 91 | 68 }}
    bit_xor {{ 80 ^ 91 ^ 68 }}
    bit_not {{ ~ ~ - ~ - 69 }}

    bool_false  {{ false }}
    bool_true   {{ true }}
    bool_concat {{ true || false }}
    bool_is     {{ 1 is true }}
    bool_eq     {{ false = 0 }}
    bool_arith  {{ true + true }}

    decode_hex_lower    {{ x'abcdef' }}
    decode_hex_upper    {{ X'AB CD EF' }}
    decode_hex_empty    {{ x'' }}
    decode_hex_unicode  {{ x'c2bf 3f' }}
    decode_hex_function {{ from_hex('ab' || 'cd') }}
    encode_hex          {{ to_hex('¿?') }}
    encode_base64       {{ to_base64(x'50B5B2B4E13199C5A43B7EF2E7155623AF928BC0C2AE13BF160923DBC3CE641AE6C67167364A6EEA57D955A7B70EF6490F502FDB425D333C96FCF7A403BBE44C') }}
    decode_base64       {{ from_base64('ULWytOExmcWkO37y5xVWI6+Si8DCrhO/' || x'0d0a' || 'Fgkj28POZBrmxnFnNkpu6lfZVae3DvZJD1Av20JdMzyW/PekA7vkTA=') }}
    encode_base64url    {{ to_base64url(x'50B5B2B4E13199C5A43B7EF2E7155623AF928BC0C2AE13BF160923DBC3CE641AE6C67167364A6EEA57D955A7B70EF6490F502FDB425D333C96FCF7A403BBE44C') }}
    decode_base64url    {{ from_base64url('ULWytOExmcWkO37y5xVWI6-Si8DCrhO_Fgkj28POZBrmxnFnNkpu6lfZVae3DvZJD1Av20JdMzyW_PekA7vkTA') }}
);
