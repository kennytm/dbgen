CREATE TABLE sbtest1 (
    id  INTEGER NOT NULL AUTO_INCREMENT,
    k   INTEGER DEFAULT '0' NOT NULL,
    c   CHAR(120) DEFAULT '' NOT NULL,
    pad CHAR(60) DEFAULT '' NOT NULL,
    PRIMARY KEY(id),
    INDEX KEY(k)
)
AS SELECT
    rownum,
    rand.int(32),
    rand.regex('([0-9]{11}-){9}[0-9]{11}'),
    rand.regex('([0-9]{11}-){4}[0-9]{11}')
FROM rand;
