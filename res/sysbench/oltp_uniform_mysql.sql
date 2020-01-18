CREATE TABLE sbtest1 (
    id  INTEGER PRIMARY KEY AUTO_INCREMENT,
        /*{{ rownum }}*/
    k   INTEGER DEFAULT '0' NOT NULL,
        /*{{ rand.range_inclusive(-0x80000000, 0x7fffffff) }}*/
    c   CHAR(120) DEFAULT '' NOT NULL,
        /*{{ rand.regex('([0-9]{11}-){9}[0-9]{11}') }}*/
    pad CHAR(60) DEFAULT '' NOT NULL,
        /*{{ rand.regex('([0-9]{11}-){4}[0-9]{11}') }}*/
    KEY(k)
);
