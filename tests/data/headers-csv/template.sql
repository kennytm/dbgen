create table a(
    col1 int {{rownum}},
    col2 int, {{rownum}}
    `col3` int {{rownum}},
    `col``4` int {{rownum}},
    "col5" int {{rownum}},
    "col""6" int {{rownum}},
    [col7] int {{rownum}},
    [col "8"] int {{rownum}}
    /* (anonymous column) */ {{rownum}}
    /* (anonymous column) */ {{rownum}}
);

{{ for each row of a generate 1 row of b }}
create table b(
    id serial primary key,
    foo numeric(40, 20) unique {{ rownum }}
);
