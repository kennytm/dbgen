create table a ({{ @a := 1 }});

/*{{ for each row of a generate 0 rows of b }}*/
create table b ({{ @b := 2 }});

/*{{ for each row of b generate 0 rows of c }}*/
create table c ({{ @a * @b }});
