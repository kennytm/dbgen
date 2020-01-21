create table result (
    {{ div(9, 4*(rownum - 2)) }}
    {{ mod(9, 4*(rownum - 2)) }}
    {{ div(-9, 4*(rownum - 2)) }}
    {{ mod(-9, 4*(rownum - 2)) }}
    {{ div(9.7, 4.1*(rownum - 2)) }}
    {{ mod(9.7, 4.1*(rownum - 2)) }}
    {{ div(-9.7, 4.1*(rownum - 2)) }}
    {{ mod(-9.7, 4.1*(rownum - 2)) }}
    {{ 9 / (4*(rownum - 2)) }}
    {{ 9.7 / (3.6*(rownum - 2)) }}
);
