{{ @data := array['One', 'Two', 'Three', 'Four', 'Five'] }}
CREATE TABLE result (
    {{
        @i := mod(rownum-1, 5);
        case when @i = 0 then @data := rand.shuffle(@data) end;
        @data[@i + 1]
    }}
);
