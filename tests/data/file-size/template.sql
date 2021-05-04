CREATE TABLE a (
    c1 text {{ rownum || 'aaaaaaaaaa' }}
);

/*{{ for each row of a generate 2 row of b }}*/
CREATE TABLE b (
    c1 text {{ rownum || 'bbbbbbbbbb' }},
    c2 text {{ subrownum || 'ccc' }}
);
