CREATE TABLE sbtest1 (
    id  INTEGER NOT NULL AUTO_INCREMENT,
        {{ rownum }}
    k   INTEGER DEFAULT '0' NOT NULL,
        {{ rand.range_inclusive(0, 0xffffffff) }}
    c   CHAR(120) DEFAULT '' NOT NULL,
        {{ rand.regex('([0-9]{11}-){9}[0-9]{11}') }}
    pad CHAR(60) DEFAULT '' NOT NULL,
        {{ rand.regex('([0-9]{11}-){4}[0-9]{11}') }}
    PRIMARY KEY(id),
    INDEX KEY(k)
);
